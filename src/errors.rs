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

impl From<std::env::VarError> for Error {
    fn from(value: std::env::VarError) -> Self {
        Error::OtherError(format!("Env var not found {:?}", value))
    }
}

impl From<std::time::SystemTimeError> for Error {
    fn from(value: std::time::SystemTimeError) -> Self {
        Error::OtherError(format!("Time error: {:?}", value))
    }
}

#[cfg(feature = "json")]
impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::OtherError(format!("JSON error: {:?}", value))
    }
}

#[cfg(feature = "gcs")]
impl From<jsonwebtoken::errors::Error> for Error {
    fn from(value: jsonwebtoken::errors::Error) -> Self {
        Error::OtherError(format!("JWT error: {:?}", value))
    }
}

#[cfg(feature = "jpeg")]
impl From<zune_jpeg::errors::DecodeErrors> for Error {
    fn from(value: zune_jpeg::errors::DecodeErrors) -> Self {
        Error::OtherError(format!("JPEG error: {:?}", value))
    }
}
