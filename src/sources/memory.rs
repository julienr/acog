use crate::errors::Error;

#[derive(Default)]
struct Stats {
    read_counts: usize,
}

pub struct MemorySource {
    buffer: Vec<u8>,
    stats: Stats,
}

impl MemorySource {
    #[allow(dead_code)] // This is used for testing
    pub fn new(buffer: Vec<u8>) -> MemorySource {
        MemorySource {
            buffer,
            stats: Default::default(),
        }
    }

    pub async fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Error> {
        let end = std::cmp::min(self.buffer.len(), offset as usize + buf.len());
        buf[..(end - offset as usize)].copy_from_slice(&self.buffer[offset as usize..end]);
        self.stats.read_counts += 1;
        Ok(end - offset as usize)
    }

    pub fn get_stats(&self) -> String {
        format!("read_counts={}", self.stats.read_counts)
    }
}
