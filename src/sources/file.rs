use std::io;
use std::io::SeekFrom;
use tokio::{fs::File, io::AsyncReadExt, io::AsyncSeekExt};

pub struct FileSource {
    pub file: File,
}

impl FileSource {
    pub async fn new(filename: &str) -> Result<FileSource, io::Error> {
        let file = File::open(filename).await?;
        Ok(FileSource { file })
    }

    /// See https://docs.rs/tokio/latest/tokio/io/trait.AsyncReadExt.html#method.read_exact
    pub async fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.file.seek(SeekFrom::Start(offset)).await?;
        self.file.read_exact(buf).await
    }
}
