use flate2::bufread::DeflateDecoder;

use crate::errors::Error;

#[derive(Debug, Clone, Copy)]
pub enum Compression {
    Raw,
    Lzw,
}

pub fn decompress_raw(data: Vec<u8>) -> Result<Vec<u8>, Error> {
    Ok(data)
}

pub fn decompress_lzw(data: Vec<u8>) -> Result<Vec<u8>, Error> {
    let mut deflateDecoder = DeflateDecoder::new(&data[..]);
    match weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 9).decode(&data) {
        Ok(decoded) => Ok(decoded),
        Err(e) => Err(Error::DecompressionError(format!(
            "lzw decompression failed: {}",
            e
        ))),
    }
}

impl Compression {
    pub fn from_compression_tag(value: u16) -> Result<Compression, Error> {
        match value {
            1 => Ok(Compression::Raw),
            5 => Ok(Compression::Lzw),
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
            Compression::Lzw => decompress_lzw(data),
        }
    }
}
