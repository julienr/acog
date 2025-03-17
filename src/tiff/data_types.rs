use super::ifd::{IFDTag, ImageFileDirectory};
use crate::errors::Error;
use crate::image;
use crate::sources::Source;

// This data type is purely used to read and should be transformed to an image::DataType
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InternalDataType {
    // Mask are packed as 8 values into a byte on disk
    Mask,
    Uint8,
    Float32,
}

fn unpack_bitmask(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len() * 8);

    for byte in bytes {
        for bit_position in 0..8 {
            let bit = (*byte >> bit_position) & 1;
            result.push(bit * 255);
        }
    }

    result
}

impl InternalDataType {
    // Some datatype (well, Mask) is stored packed as 8 values per byte. So we need an
    // "unpack" step first
    pub fn unpack_bytes(&self, data: &[u8]) -> Vec<u8> {
        match self {
            InternalDataType::Mask => unpack_bitmask(data),
            InternalDataType::Uint8 | InternalDataType::Float32 => data.to_vec(),
        }
    }

    // The corresponding data type after the data has gone through `unpack_bytes`
    pub fn unpacked_type(&self) -> image::DataType {
        match self {
            InternalDataType::Mask | InternalDataType::Uint8 => image::DataType::Uint8,
            InternalDataType::Float32 => image::DataType::Float32,
        }
    }
}

fn check_all_same(numbers: &[u16]) -> Result<u16, Error> {
    if numbers.is_empty() {
        return Err(Error::InvalidData(
            "Expected at least one value, got an empty list".to_string(),
        ));
    }

    let first_value = numbers[0];
    for num in numbers {
        if *num != first_value {
            return Err(Error::InvalidData(format!(
                "Expected same value in whole list, go {:?}",
                numbers
            )));
        }
    }

    Ok(first_value)
}

pub async fn data_type_from_ifd(
    ifd: &ImageFileDirectory,
    source: &mut Source,
) -> Result<InternalDataType, Error> {
    let sample_format = check_all_same(
        &ifd.get_vec_short_tag_value(source, IFDTag::SampleFormat)
            .await?,
    )?;
    let bits_per_sample = check_all_same(
        &ifd.get_vec_short_tag_value(source, IFDTag::BitsPerSample)
            .await?,
    )?;
    if sample_format == 1 {
        if bits_per_sample == 1 {
            Ok(InternalDataType::Mask)
        } else if bits_per_sample == 8 {
            Ok(InternalDataType::Uint8)
        } else {
            Err(Error::UnsupportedDataType(format!(
                "SampleFormat={}, BitsPerSample={}",
                sample_format, bits_per_sample
            )))
        }
    } else if sample_format == 2 {
        // int
        Err(Error::UnsupportedDataType(format!(
            "SampleFormat={}",
            sample_format
        )))
    } else if sample_format == 3 {
        // float
        if bits_per_sample == 32 {
            Ok(InternalDataType::Float32)
        } else {
            Err(Error::UnsupportedDataType(format!(
                "SampleFormat={}, BitsPerSample={}",
                sample_format, bits_per_sample
            )))
        }
    } else {
        Err(Error::UnsupportedDataType(format!(
            "SampleFormat={}",
            sample_format
        )))
    }
}
