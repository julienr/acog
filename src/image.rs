use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataType {
    Uint8,
    Float32,
}

impl DataType {
    pub fn size_bytes(&self) -> usize {
        match self {
            DataType::Uint8 => 1,
            DataType::Float32 => 4,
        }
    }
}

pub struct ImageBuffer {
    pub width: usize,
    pub height: usize,
    pub nbands: usize,
    pub has_alpha: bool,
    pub data_type: DataType,
    // The image data stored in row-major order, packed by pixel
    pub data: Vec<u8>,
}

impl ImageBuffer {
    // Converts this image buffer to a RGB image buffer by:
    // - selecting 3 bands
    // - normalizing them with vmin/vmax
    pub fn to_rgb(&self, bands: &[usize; 3], vmin: f64, vmax: f64) -> Result<ImageBuffer, Error> {
        let mut out_data = vec![0u8; self.width * self.height * 3];
        match self.data_type {
            DataType::Uint8 => todo!("Uint8 to RGB conversion not implemented"),
            DataType::Float32 => {
                for i in 0..self.height {
                    for j in 0..self.width {
                        for (bi, b) in bands.iter().enumerate() {
                            let offset = i * self.width * self.nbands * self.data_type.size_bytes()
                                + j * self.nbands * self.data_type.size_bytes()
                                + b * self.data_type.size_bytes();
                            let v: f32 = f32::from_le_bytes(
                                self.data[offset..offset + 4].try_into().unwrap(),
                            );
                            let out_offset = i * self.width * 3 + j * 3 + bi;
                            out_data[out_offset] =
                                (255.0f64 * (v as f64 - vmin) / (vmax - vmin)) as u8;
                        }
                    }
                }
            }
        }
        Ok(ImageBuffer {
            width: self.width,
            height: self.height,
            nbands: 3,
            has_alpha: false,
            data_type: DataType::Uint8,
            data: out_data,
        })
    }

    pub fn drop_alpha(self) -> ImageBuffer {
        if self.has_alpha {
            let visual_bands = self.nbands - 1;
            let data = self
                .data
                .chunks(self.nbands * self.data_type.size_bytes())
                .flat_map(|chunk| chunk[..visual_bands * self.data_type.size_bytes()].to_vec())
                .collect();
            ImageBuffer {
                width: self.width,
                height: self.height,
                nbands: visual_bands,
                data_type: self.data_type,
                has_alpha: false,
                data,
            }
        } else {
            self
        }
    }
}

pub fn stack(image1: &ImageBuffer, image2: &ImageBuffer) -> Result<ImageBuffer, Error> {
    if image1.width != image2.width || image1.height != image2.height {
        return Err(Error::OtherError(format!(
            "incompatible image dimensions: ({}, {}) != ({}, {})",
            image1.width, image1.height, image2.width, image2.height,
        )));
    }
    if image1.data_type != image2.data_type {
        return Err(Error::OtherError(format!(
            "incompatible data types: {:?} != {:?}",
            image1.data_type, image2.data_type,
        )));
    }
    if image1.has_alpha {
        return Err(Error::OtherError(
            "doesn't support stacking first image with alpha".to_string(),
        ));
    }
    let mut out_data = vec![
        0u8;
        image1.width
            * image1.height
            * (image1.nbands + image2.nbands)
            * image1.data_type.size_bytes()
    ];
    for i in 0..image1.height {
        for j in 0..image1.width {
            let out_offset = i * image1.width * (image1.nbands + image2.nbands)
                + j * (image1.nbands + image2.nbands);
            out_data[out_offset..out_offset + image1.nbands].copy_from_slice({
                let in_offset = i * image1.width * image1.nbands + j * image1.nbands;
                &image1.data[in_offset..in_offset + image1.nbands]
            });
            out_data[out_offset + image1.nbands..out_offset + image1.nbands + image2.nbands]
                .copy_from_slice({
                    let in_offset = i * image2.width * image2.nbands + j * image2.nbands;
                    &image2.data[in_offset..in_offset + image2.nbands]
                });
        }
    }
    Ok(ImageBuffer {
        width: image1.width,
        height: image2.height,
        nbands: image1.nbands + image2.nbands,
        has_alpha: image2.has_alpha,
        data_type: image1.data_type,
        data: out_data,
    })
}

#[cfg(test)]
mod tests {
    use super::DataType;
    use super::ImageBuffer;
    use super::{drop_alpha, stack};

    #[test]
    fn test_stack() {
        let width = 32;
        let height = 16;
        let image1 = {
            let mut data = vec![0u8; width * height * 3];
            // Set pixel at (x=10, y=4) to RGB = (0, 42, 0)
            data[4 * 32 * 3 + 10 * 3 + 1] = 42;
            // Set pixel at (x=31, y=15) to RGB = (0, 0, 56)
            data[15 * 32 * 3 + 31 * 3 + 2] = 56;
            ImageBuffer {
                width,
                height,
                nbands: 3,
                has_alpha: false,
                data_type: DataType::Uint8,
                data,
            }
        };
        let image2 = {
            let mut data = vec![0u8; width * height * 1];
            // Set pixel at (x=10, y=4) to val = 7
            data[4 * 32 + 10] = 7;
            ImageBuffer {
                width,
                height,
                nbands: 1,
                has_alpha: true,
                data_type: DataType::Uint8,
                data,
            }
        };
        let res = stack(&image1, &image2).unwrap();
        assert_eq!(res.width, 32);
        assert_eq!(res.height, 16);
        assert_eq!(res.nbands, 4);
        assert_eq!(res.has_alpha, true);
        assert_eq!(res.data_type, DataType::Uint8);
        // RGB values from image1
        {
            let offset = 4 * width * 4 + 10 * 4;
            assert_eq!(res.data[offset..offset + 3], [0, 42, 0]);
        }
        {
            let offset = 15 * width * 4 + 31 * 4;
            assert_eq!(res.data[offset..offset + 3], [0, 0, 56]);
        }
        // alpha from image2
        assert_eq!(res.data[4 * width * 4 + 10 * 4 + 3], 7);
    }

    #[test]
    fn test_to_rgb() {
        let image1 = {
            // 5 bands
            let width = 32;
            let height = 8;
            let nbands = 5;
            let byte_size = 4;
            let mut data = vec![0u8; width * height * nbands * byte_size];
            // Set pixel at (x=10, y=4) to values = (0, 0, 0, 1.2, 0)
            let offset = 4 * width * nbands * byte_size + 10 * nbands * byte_size + 3 * byte_size;
            data[offset..offset + 4].copy_from_slice(&1.2f32.to_le_bytes());
            ImageBuffer {
                width,
                height,
                nbands,
                has_alpha: false,
                data_type: DataType::Float32,
                data,
            }
        };
        let res = image1.to_rgb(&[2, 3, 4], 0f64, 2.4f64).unwrap();
        assert_eq!(res.width, 32);
        assert_eq!(res.height, 8);
        assert_eq!(res.nbands, 3);
        assert_eq!(res.has_alpha, false);
        assert_eq!(res.data_type, DataType::Uint8);
        // Since we select band [2, 3, 4] and have vmax=2.4, (0, 0, 0, 1.2, 0)
        // turns into (0, 127, 0) after RGB conversion
        let offset = 4 * 32 * 3 + 10 * 3;
        assert_eq!(res.data[offset..offset + 3], [0, 127, 0]);
    }

    #[test]
    fn test_drop_alpha() {
        let width = 32;
        let height = 16;
        let image1 = {
            let mut data = vec![0u8; width * height * 4];
            // Set pixel at (x=10, y=4) to RGB = (41, 42, 43)
            data[4 * 32 * 4 + 10 * 4 + 0] = 41;
            data[4 * 32 * 4 + 10 * 4 + 1] = 42;
            data[4 * 32 * 4 + 10 * 4 + 2] = 43;
            // Set pixel at (x=31, y=15) to RGB = (0, 0, 56)
            data[15 * 32 * 4 + 31 * 4 + 2] = 56;
            ImageBuffer {
                width,
                height,
                nbands: 4,
                has_alpha: true,
                data_type: DataType::Uint8,
                data,
            }
        };
        let res = drop_alpha(image1);
        assert_eq!(res.width, 32);
        assert_eq!(res.height, 16);
        assert_eq!(res.nbands, 3);
        assert_eq!(res.has_alpha, false);
        assert_eq!(res.data_type, DataType::Uint8);
        {
            let offset = 4 * width * 3 + 10 * 3;
            assert_eq!(res.data[offset..offset + 3], [41, 42, 43]);
        }
        {
            let offset = 15 * width * 3 + 31 * 3;
            assert_eq!(res.data[offset..offset + 3], [0, 0, 56]);
        }
    }
}
