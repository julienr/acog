use reqwest::header::ToStrError;

use crate::tiff::geo_keys::{KeyID, KeyValue};
use crate::tiff::ifd::{IFDTag, IFDValue};
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
    DecompressionError(String),
    GeoKeyHasWrongType(KeyID, KeyValue),
    UnsupportedProjection(String),
    UnsupportedCompression(String),
    UnsupportedUnit(String),
    OutOfBoundsRead(String),
    UnsupportedCOG(String),
    ReqwestError(reqwest::Error),
    ProjError(proj::Error),
    OtherError(String),
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Error::IO(value)
    }
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Error::ReqwestError(value)
    }
}

impl From<ToStrError> for Error {
    fn from(value: ToStrError) -> Self {
        Error::OtherError(format!("ToStrError: {:?}", value))
    }
}

impl From<proj::Error> for Error {
    fn from(value: proj::Error) -> Self {
        Error::ProjError(value)
    }
}
