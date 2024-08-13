use crate::errors::Error;

mod deflate;

#[derive(Debug, Clone, Copy)]
pub enum Compression {
    Raw,
    Deflate,
}

pub fn decompress_raw(data: Vec<u8>) -> Result<Vec<u8>, Error> {
    Ok(data)
}

impl Compression {
    pub fn from_compression_tag(value: u16) -> Result<Compression, Error> {
        // https://www.awaresystems.be/imaging/tiff/tifftags/compression.html
        match value {
            1 => Ok(Compression::Raw),
            // Using COMPRESS=DEFLATE with GDAL generates tag 8 which is actually "Adobe deflate"
            8 => Ok(Compression::Deflate),
            _ => Err(Error::UnsupportedCompression(format!(
                "Unsupported compression {}",
                value
            ))),
        }
    }

    // TODO: Should we expose a weezl-like `into_vec` instead ? That would allow reducing allocations
    // from the caller
    pub fn decompress(&self, data: Vec<u8>) -> Result<Vec<u8>, Error> {
        match self {
            Compression::Raw => decompress_raw(data),
            Compression::Deflate => deflate::decompress_deflate(data),
        }
    }
}
