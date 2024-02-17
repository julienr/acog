use std::io;

pub struct MemorySource {
    buffer: Vec<u8>,
}

impl MemorySource {
    pub fn new(buffer: Vec<u8>) -> MemorySource {
        MemorySource { buffer }
    }

    /// See https://docs.rs/tokio/latest/tokio/io/trait.AsyncReadExt.html#method.read
    pub async fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, io::Error> {
        let end = std::cmp::min(self.buffer.len(), offset as usize + buf.len());
        buf[..(end - offset as usize)].copy_from_slice(&self.buffer[offset as usize..end as usize]);
        /*
        println!(
            "memory: buffer.len={}, offset={}, buf.len={}, bytes_read={}",
            self.buffer.len(),
            offset,
            buf.len(),
            end - offset as usize
        );
        */
        Ok(end - offset as usize)
    }
}
