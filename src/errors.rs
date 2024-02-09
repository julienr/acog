use crate::tiff::ifd::{IFDTag, IFDValue};
use std::io;

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    InvalidData(String),
    RequiredTagNotFound(IFDTag),
    TagHasWrongType(IFDTag, IFDValue),
    UnsupportedTagValue(IFDTag, String),
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Error::IO(value)
    }
}
