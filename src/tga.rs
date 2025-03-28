// Target simple RGBA read & write support
// https://paulbourke.net/dataformats/tga/
use crate::image::{DataType, ImageBuffer};
use crate::Error;
use std::io::{BufReader, Read, Write};

pub fn write_to_tga(filename: &str, img: &ImageBuffer) -> Result<(), Error> {
    if img.data_type != DataType::Uint8 {
        return Err(Error::OtherError(format!(
            "Only uint8 images are supported, got dtype={:?}",
            img.data_type
        )));
    }
    if img.nbands != 4 && img.nbands != 3 {
        return Err(Error::OtherError(format!(
            "Only RGB or RGBA images are supported, got nbands={}",
            img.nbands
        )));
    }

    let mut file = std::fs::File::create(filename)?;
    let mut img_spec = [0u8; 18];
    // ID length
    img_spec[0] = 0;
    // Color map type
    img_spec[1] = 0;
    // Image type (unmapped RGB)
    img_spec[2] = 2;
    // Color map spec - ignored for unmapped RGB
    img_spec[3..8].copy_from_slice(&[0u8; 5]);
    // Image spec: X origin as little endian uint16
    img_spec[8..10].copy_from_slice(&0u16.to_le_bytes());
    // Image spec: Y origin as little endian uint16
    img_spec[10..12].copy_from_slice(&0u16.to_le_bytes());
    // Image spec: width as little endian uint16
    {
        let width_u16: u16 = img.width.try_into()?;
        img_spec[12..14].copy_from_slice(&width_u16.to_le_bytes());
    }
    // Image spec: width as little endian uint16
    {
        let height_u16: u16 = img.height.try_into()?;
        img_spec[14..16].copy_from_slice(&height_u16.to_le_bytes());
    }
    // 32 bpp
    img_spec[16] = 32;
    // Image spec: Image descriptor byte
    // - Bits 0-3 give alpha channel depth which is 8
    // - Bit 4 must be 0
    // - Bit 5 is screen origin, which we set to 1 (upper left)
    img_spec[17] = 8 | (1 << 5);

    file.write_all(&img_spec)?;
    // Image data stored as BGRA on disk
    // https://paulbourke.net/dataformats/tga/
    for i in 0..img.height {
        for j in 0..img.width {
            let offset = i * img.width * img.nbands + j * img.nbands;
            let r = img.data[offset];
            let g = img.data[offset + 1];
            let b = img.data[offset + 2];
            let a = if img.nbands == 4 {
                img.data[offset + 3]
            } else {
                255
            };
            file.write_all(&[b, g, r, a])?;
        }
    }
    Ok(())
}

fn check_bytes(actual: &[u8], expected: &[u8]) -> Result<(), Error> {
    if actual != expected {
        return Err(Error::OtherError(format!(
            "Invalid data: expected {:?}, got {:?}",
            expected, actual
        )));
    }
    Ok(())
}

pub fn read_tga(filename: &str) -> Result<ImageBuffer, Error> {
    let f = std::fs::File::open(filename)?;
    let mut r = BufReader::new(f);
    let mut img_spec = [0u8; 18];
    r.read_exact(&mut img_spec)?;
    check_bytes(&img_spec[0..3], &[0, 0, 2])?;
    check_bytes(&img_spec[3..8], &[0u8; 5])?;
    check_bytes(&img_spec[8..10], &[0, 0])?;
    check_bytes(&img_spec[10..12], &[0, 0])?;
    let width: usize = u16::from_le_bytes([img_spec[12], img_spec[13]]).into();
    let height: usize = u16::from_le_bytes([img_spec[14], img_spec[15]]).into();
    check_bytes(&img_spec[16..17], &[32])?;
    check_bytes(&img_spec[17..18], &[8 | (1 << 5)])?;

    let mut data: Vec<u8> = vec![];
    r.read_to_end(&mut data)?;
    {
        let expected_size = width * height * 4;
        if data.len() != expected_size {
            return Err(Error::OtherError(format!(
                "expected_size={}, got={}",
                expected_size,
                data.len()
            )));
        }
    }
    // Image data is stored as BGRA => convert to RGBA
    for i in 0..height {
        for j in 0..width {
            let offset = i * width * 4 + j * 4;
            let b = data[offset];
            let g = data[offset + 1];
            let r = data[offset + 2];
            let a = data[offset + 3];
            data[offset..offset + 4].copy_from_slice(&[r, g, b, a]);
        }
    }

    Ok(ImageBuffer {
        width,
        height,
        has_alpha: true,
        nbands: 4,
        data_type: DataType::Uint8,
        data,
    })
}

#[cfg(test)]
mod tests {
    use crate::image::{DataType, ImageBuffer};

    #[test]
    fn test_write_read_tga_rgb() {
        let data = vec![0u8, 0u8, 0u8, 255u8, 255u8, 255u8];
        // We always write as RGBA, so just expected alpha=255 here
        let expected_data = vec![0u8, 0u8, 0u8, 255u8, 255u8, 255u8, 255u8, 255u8];
        super::write_to_tga(
            "/tmp/test.ppm",
            &ImageBuffer {
                width: 2,
                height: 1,
                nbands: 3,
                has_alpha: false,
                data_type: DataType::Uint8,
                data: data.clone(),
            },
        )
        .unwrap();
        let actual_img = super::read_tga("/tmp/test.ppm").unwrap();
        assert_eq!(actual_img.width, 2);
        assert_eq!(actual_img.height, 1);
        assert_eq!(actual_img.nbands, 4);
        assert!(actual_img.has_alpha);
        assert_eq!(actual_img.data, expected_data);
    }

    #[test]
    fn test_write_read_tga_rgba() {
        let data = vec![0u8, 0u8, 0u8, 127u8, 255u8, 255u8, 255u8, 0u8];
        super::write_to_tga(
            "/tmp/test.ppm",
            &ImageBuffer {
                width: 2,
                height: 1,
                nbands: 4,
                has_alpha: true,
                data_type: DataType::Uint8,
                data: data.clone(),
            },
        )
        .unwrap();
        let actual_img = super::read_tga("/tmp/test.ppm").unwrap();
        assert_eq!(actual_img.width, 2);
        assert_eq!(actual_img.height, 1);
        assert_eq!(actual_img.nbands, 4);
        assert!(actual_img.has_alpha);
        assert_eq!(actual_img.data, data);
    }
}
