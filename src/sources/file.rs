use crate::errors::Error;
use std::io;
use std::io::SeekFrom;
use tokio::{fs::File, io::AsyncReadExt, io::AsyncSeekExt};

#[derive(Default)]
struct Stats {
    read_counts: usize,
}

pub struct FileSource {
    pub file: File,
    stats: Stats,
}

impl FileSource {
    pub async fn new(filename: &str) -> Result<FileSource, io::Error> {
        let file = File::open(filename).await?;
        Ok(FileSource {
            file,
            stats: Default::default(),
        })
    }

    pub async fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Error> {
        self.file.seek(SeekFrom::Start(offset)).await?;
        self.stats.read_counts += 1;
        Ok(self.file.read(buf).await?)
    }

    pub fn get_stats(&self) -> String {
        format!("read_counts={}", self.stats.read_counts)
    }
}
