use super::Error;
use weezl::decode::Decoder;

pub fn decompress_lzw(data: Vec<u8>) -> Result<Vec<u8>, Error> {
    match Decoder::new(weezl::BitOrder::Msb, 9).decode(&data) {
        Ok(v) => Ok(v),
        Err(e) => Err(Error::DecompressionError(format!(
            "decompression error: {}",
            e
        ))),
    }
}
