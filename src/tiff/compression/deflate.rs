use super::Error;
use flate2::bufread::DeflateDecoder;
use std::io::Read;

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
