use crate::tiff::ifd::{IFDTag, IFDValue};
use crate::tiff::geo_keys::{KeyID, KeyValue};
use std::io;

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    SourceError(String),
    InvalidData(String),
    RequiredTagNotFound(IFDTag),
    TagHasWrongType(IFDTag, IFDValue),
    UnsupportedTagValue(IFDTag, String),
    NotACOG(String),
    RequiredGeoKeyNotFound(KeyID),
    GeoKeyHasWrongType(KeyID, KeyValue),
    UnsupportedProjection(String),
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Error::IO(value)
    }
}
