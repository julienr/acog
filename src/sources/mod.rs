use std::fmt;

mod file;
mod memory;
mod s3;

pub use file::FileSource;
pub use memory::MemorySource;
pub use s3::S3Source;

use crate::errors::Error;

pub enum Source {
    File(FileSource),
    S3(S3Source),
    Memory(MemorySource),
}

impl fmt::Debug for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(_) => f.debug_tuple("File").finish(),
            Self::S3(_) => f.debug_tuple("S3").finish(),
            Self::Memory(_) => f.debug_tuple("Memory").finish(),
        }
    }
}

impl Source {
    /// See https://docs.rs/tokio/latest/tokio/io/trait.AsyncReadExt.html#method.read_exact
    pub async fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Error> {
        match self {
            Source::File(s) => s.read_exact(offset, buf).await,
            Source::S3(s) => s.read_exact(offset, buf).await,
            Source::Memory(s) => s.read_exact(offset, buf).await,
        }
    }

    pub fn get_stats(&self) -> String {
        match self {
            Source::File(s) => s.get_stats(),
            Source::S3(s) => s.get_stats(),
            Source::Memory(s) => s.get_stats(),
        }
    }
}
