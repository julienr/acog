use crate::{tiff::data_types::DataType, Error};

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
            DataType::Mask => todo!("Mask to RGB conversion not implemented"),
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
}

/*
pub fn stack(image1: &ImageBuffer, image2: &ImageBuffer) -> ImageBuffer {
    if image1.data_type
}
*/
