use std::io::Read;

use flate2::bufread::DeflateDecoder;

use crate::errors::Error;

#[derive(Debug, Clone, Copy)]
pub enum Compression {
    Raw,
    Deflate,
}

pub fn decompress_raw(data: Vec<u8>) -> Result<Vec<u8>, Error> {
    Ok(data)
}

pub fn decompress_deflate(data: Vec<u8>) -> Result<Vec<u8>, Error> {
    // As per the Adobe deflate documentation, the compressed data should start with a header:
    // https://www.awaresystems.be/imaging/tiff/specification/TIFFphotoshop.pdf
    // Or section 2.2 of the zlib RFC
    // https://www.rfc-editor.org/rfc/rfc1950
    let header = &data[0..2];
    if header[0] & 0xF != 8 {
        return Err(Error::DecompressionError(format!(
            "Invalid deflate header: {:?}",
            header,
        )));
    }
    let mut decoder = DeflateDecoder::new(&data[2..]);
    let mut out: Vec<u8> = vec![];
    match decoder.read_to_end(&mut out) {
        Ok(_nbytes) => Ok(out),
        Err(e) => Err(Error::DecompressionError(format!(
            "decompression error: {}",
            e
        ))),
    }
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
            Compression::Deflate => decompress_deflate(data),
        }
    }
}
