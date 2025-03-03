use super::ifd::{IFDTag, IFDValue, ImageFileDirectory};
use super::tags::PhotometricInterpretation;
use crate::errors::Error;
use crate::sources::Source;

mod deflate;
mod jpeg;

#[derive(Clone)]
pub enum Compression {
    Raw,
    Deflate,
    Jpeg(jpeg::Decompressor),
}

impl std::fmt::Debug for Compression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw => write!(f, "Raw"),
            Self::Deflate => write!(f, "Deflate"),
            Self::Jpeg(_) => write!(f, "Jpeg"),
        }
    }
}

pub fn decompress_raw(data: Vec<u8>) -> Result<Vec<u8>, Error> {
    Ok(data)
}

async fn jpeg_from_ifd(
    source: &mut Source,
    ifd: &ImageFileDirectory,
) -> Result<jpeg::Decompressor, Error> {
    let photo_interp = PhotometricInterpretation::read_from_ifd(source, ifd).await?;
    if photo_interp != PhotometricInterpretation::YCbCr {
        return Err(Error::DecompressionError(format!(
            "JPEG only support YCbCr photometric interpretation, got {:?}",
            photo_interp
        )));
    }
    let jpeg_tables = ifd
        .get_vec_undefined_raw_byte_tag_value(source, IFDTag::JpegTables)
        .await?;
    jpeg::Decompressor::new(&jpeg_tables)
}

impl Compression {
    pub async fn from_ifd(
        source: &mut Source,
        ifd: &ImageFileDirectory,
    ) -> Result<Compression, Error> {
        let compression_type = match ifd.get_tag_value(source, IFDTag::Compression).await? {
            IFDValue::Short(v) => v[0],
            value => return Err(Error::TagHasWrongType(IFDTag::Compression, value)),
        };
        // https://www.awaresystems.be/imaging/tiff/tifftags/compression.html
        match compression_type {
            1 => Ok(Compression::Raw),
            // Using COMPRESS=DEFLATE with GDAL generates tag 8 which is actually "Adobe deflate"
            8 => Ok(Compression::Deflate),
            7 => Ok(Compression::Jpeg(jpeg_from_ifd(source, ifd).await?)),
            _ => Err(Error::UnsupportedCompression(format!(
                "Unsupported compression {}",
                compression_type
            ))),
        }
    }

    // TODO: Should we expose a weezl-like `into_vec` instead ? That would allow reducing allocations
    // from the caller
    pub fn decompress(&self, data: Vec<u8>, width: usize, height: usize) -> Result<Vec<u8>, Error> {
        match self {
            Compression::Raw => decompress_raw(data),
            Compression::Deflate => deflate::decompress_deflate(data),
            Compression::Jpeg(decompressor) => Ok(decompressor.decompress(data, width, height)?),
        }
    }
}
