use std::io;
use tokio::{fs::File, io::AsyncReadExt};

pub struct S3Source {
    pub file: File,
}

impl S3Source {
    pub async fn new(filename: &str) -> Result<S3Source, io::Error> {
        // TODO: S3 instead of file
        let file = File::open(filename).await?;
        Ok(S3Source { file })
    }

    /// See https://docs.rs/tokio/latest/tokio/io/trait.AsyncReadExt.html#method.read
    pub async fn read(&mut self, _offset: u64, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.file.read(buf).await
    }
}
