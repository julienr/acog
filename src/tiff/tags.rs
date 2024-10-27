use crate::{sources::Source, Error};

use super::ifd::{IFDTag, IFDValue, ImageFileDirectory};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhotometricInterpretation {
    BlackIsZero,
    Rgb,
    Mask,
    YCbCr,
}

impl PhotometricInterpretation {
    pub fn decode(v: u16) -> Result<PhotometricInterpretation, Error> {
        match v {
            1 => Ok(PhotometricInterpretation::BlackIsZero),
            2 => Ok(PhotometricInterpretation::Rgb),
            4 => Ok(PhotometricInterpretation::Mask),
            6 => Ok(PhotometricInterpretation::YCbCr),
            v => Err(Error::UnsupportedTagValue(
                super::ifd::IFDTag::PhotometricInterpretation,
                format!("{:?}", v),
            )),
        }
    }

    pub async fn read_from_ifd(
        source: &mut Source,
        ifd: &ImageFileDirectory,
    ) -> Result<PhotometricInterpretation, Error> {
        match ifd
            .get_tag_value(source, IFDTag::PhotometricInterpretation)
            .await?
        {
            IFDValue::Short(v) => match v[..] {
                [v0] => PhotometricInterpretation::decode(v0),
                _ => Err(Error::UnsupportedTagValue(
                    IFDTag::PhotometricInterpretation,
                    format!("{:?}", v),
                )),
            },
            value => Err(Error::TagHasWrongType(
                IFDTag::PhotometricInterpretation,
                value,
            )),
        }
    }
}
