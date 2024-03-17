use crate::errors::Error;

pub struct MemorySource {
    buffer: Vec<u8>,
}

impl MemorySource {
    pub fn new(buffer: Vec<u8>) -> MemorySource {
        MemorySource { buffer }
    }

    /// See https://docs.rs/tokio/latest/tokio/io/trait.AsyncReadExt.html#method.read_exact
    pub async fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Error> {
        let end = std::cmp::min(self.buffer.len(), offset as usize + buf.len());
        buf[..(end - offset as usize)].copy_from_slice(&self.buffer[offset as usize..end as usize]);
        Ok(end - offset as usize)
    }
}
