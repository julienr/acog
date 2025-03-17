/// Utility functions to read/write ppm - only used for tests so this isn't meant as a general
/// purpose ppm library
use crate::image::{DataType, ImageBuffer};
use crate::Error;
use std::io::{BufReader, Read, Write};
use std::str;

pub fn write_to_ppm(filename: &str, img: &ImageBuffer) -> Result<(), Error> {
    if img.data_type != DataType::Uint8 {
        return Err(Error::OtherError(format!(
            "Only uint8 images are supported, got dtype={:?}",
            img.data_type
        )));
    }
    if img.nbands != 3 {
        return Err(Error::OtherError(format!(
            "Only RGB images are supported, got nbands={}",
            img.nbands
        )));
    }

    let mut file = std::fs::File::create(filename)?;
    file.write_all(format!("P6 {} {} 255\n", img.width, img.height).as_bytes())?;
    file.write_all(&img.data)?;
    Ok(())
}

impl From<std::num::ParseIntError> for Error {
    fn from(value: std::num::ParseIntError) -> Self {
        Error::OtherError(format!("Failed to parse int: {:?}", value))
    }
}

pub fn read_ppm(filename: &str) -> Result<ImageBuffer, Error> {
    let f = std::fs::File::open(filename)?;
    let mut r = BufReader::new(f);
    // Magic number
    {
        let mut magic = vec![0u8; 2];
        r.read_exact(&mut magic)?;
        if magic[0] != b'P' && magic[1] != b'6' {
            return Err(Error::OtherError(format!(
                "Unexpected magic number: {:?}",
                magic
            )));
        }
    }
    // Rest of header line
    let (width, height) = {
        let mut line = vec![0u8; 0];
        let mut buf = vec![0u8; 1];
        loop {
            r.read_exact(&mut buf)?;
            if buf[0] == b'\n' {
                break;
            }
            line.push(buf[0]);
        }
        let line = match str::from_utf8(&line) {
            Ok(v) => v,
            Err(e) => {
                return Err(Error::OtherError(format!(
                    "Failed to parse header: {:?}",
                    e
                )))
            }
        };
        let splits: Vec<&str> = line.trim().split(' ').collect();
        if splits.len() != 3 {
            return Err(Error::OtherError(format!(
                "Failed to parse header: '{}', len={}",
                line,
                splits.len()
            )));
        }
        let width = splits[0].parse::<usize>()?;
        let height = splits[1].parse::<usize>()?;
        let max_val = splits[2].parse::<usize>()?;
        if max_val != 255 {
            return Err(Error::OtherError(format!("Invalid max_val={}", max_val)));
        }
        (width, height)
    };
    // Read the data
    let mut data = vec![0u8; width * height * 3];
    r.read_exact(&mut data)?;
    Ok(ImageBuffer {
        width,
        height,
        nbands: 3,
        has_alpha: false,
        data_type: DataType::Uint8,
        data,
    })
}

#[cfg(test)]
mod tests {
    use crate::image::{DataType, ImageBuffer};

    #[test]
    fn test_write_read_ppm() {
        let data = vec![0u8, 0u8, 0u8, 255u8, 255u8, 255u8];
        super::write_to_ppm(
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
        let actual_img = super::read_ppm("/tmp/test.ppm").unwrap();
        assert_eq!(actual_img.width, 2);
        assert_eq!(actual_img.height, 1);
        assert_eq!(actual_img.nbands, 3);
        assert_eq!(actual_img.data, data);
    }
}
