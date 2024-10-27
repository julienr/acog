use super::ifd::{IFDTag, ImageFileDirectory};
use crate::errors::Error;
use crate::sources::Source;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataType {
    Mask,
    Uint8,
    Float32,
}

impl DataType {
    pub fn size_bytes(&self) -> usize {
        match self {
            DataType::Mask => 1,
            DataType::Uint8 => 1,
            DataType::Float32 => 4,
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
) -> Result<DataType, Error> {
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
            Ok(DataType::Mask)
        } else if bits_per_sample == 8 {
            Ok(DataType::Uint8)
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
            Ok(DataType::Float32)
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
