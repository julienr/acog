use std::{fmt, io::ErrorKind};

mod auth;
mod file;
#[cfg(feature = "gcs")]
mod gcs;
mod memory;
mod s3;

pub use file::FileSource;
pub use gcs::GCSSource;
pub use memory::MemorySource;
pub use s3::S3Source;
use std::collections::HashMap;

use crate::errors::Error;
use std::io;

enum SourceKind {
    File(FileSource),
    S3(S3Source),
    Gcs(GCSSource),
    #[allow(dead_code)] // This is used for testing
    Memory(MemorySource),
}

impl fmt::Debug for SourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(_) => f.debug_tuple("File").finish(),
            Self::S3(_) => f.debug_tuple("S3").finish(),
            Self::Gcs(_) => f.debug_tuple("GCS").finish(),
            Self::Memory(_) => f.debug_tuple("Memory").finish(),
        }
    }
}

impl SourceKind {
    /// This tries to read the given buffer at the given offset. If EOF is reached, this will
    /// return Ok(n) where n < buf.len()
    async fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Error> {
        match self {
            SourceKind::File(s) => s.read(offset, buf).await,
            SourceKind::S3(s) => s.read(offset, buf).await,
            SourceKind::Gcs(s) => s.read(offset, buf).await,
            SourceKind::Memory(s) => s.read(offset, buf).await,
        }
    }

    /// Reads exactly the given buffer from the given offset. This returns an Err(ErrorKind::UnexpectedEof)
    /// if Eof is reached while reading. If this returns Ok(), it is guaranteed the whole buffer has been read
    /// See https://docs.rs/tokio/latest/tokio/io/trait.AsyncReadExt.html#method.read_exact
    pub async fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), Error> {
        let bytes_count = self.read(offset, buf).await?;
        if bytes_count < buf.len() {
            Err(Error::IO(io::Error::from(ErrorKind::UnexpectedEof)))
        } else {
            Ok(())
        }
    }

    pub fn get_stats(&self) -> String {
        match self {
            SourceKind::File(s) => s.get_stats(),
            SourceKind::S3(s) => s.get_stats(),
            SourceKind::Gcs(s) => s.get_stats(),
            SourceKind::Memory(s) => s.get_stats(),
        }
    }
}

const CHUNK_SIZE: usize = 16384; // 16 kB, like GDAL `CPL_VSIL_CURL_CHUNK_SIZE`

const MAX_CACHED_CHUNKS: usize = 100;

/// Sources support chunked reading mode with caching and direct reading.
/// - Chunked reading with caching should be uses to tread the header + IFDs
/// - Direct reading should be used to read image data
///
/// This adds a layer of buffering when reading from a Source. The intent here is to minimize the
/// number of actual source reads (which result in disk IO or HTTP requests) when reading the IFD/TIFF
/// header.
/// One of the design goal of COG is that all of the IFD data (except some offsets/sizes values) are
/// packed at the beginning of the file such that it can be read in a single HTTP range request.
/// For example GDAL will start by reading the first 16KB of the file when it's opened and this
/// most likely contain the whole IFD.
///
/// Note that what you'd really like is for the TIFF file to start with the size of the IFDs and read
/// exactly that. That's not how TIFF work though.
///
/// One the other hand when reading actual overview tile data, we do not need buffered reading because
/// we're usually reading exactly what we need.
struct ChunkCache {
    // Maps a chunk index to the chunk data. Note that the last chunk will still have CHUNK_SIZE
    // data, but data past `source_len` will be filled with 0
    chunks_cache: HashMap<u32, [u8; CHUNK_SIZE]>,
    // Once we have reached EOF, we store the source len here
    source_len: Option<u64>,
}

impl fmt::Debug for ChunkCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufferedSourceReader")
            .field("source_len", &self.source_len)
            .finish()
    }
}

impl ChunkCache {
    pub fn new() -> Self {
        ChunkCache {
            chunks_cache: HashMap::new(),
            source_len: None,
        }
    }

    async fn read_chunk(
        &mut self,
        source_kind: &mut SourceKind,
        chunk_index: u32,
    ) -> Result<&[u8; CHUNK_SIZE], Error> {
        if self.chunks_cache.len() >= MAX_CACHED_CHUNKS {
            // Here, a LRU cache would probably be a better idea. For now we just evict randomly
            // a page for simplicity's sake
            let key = *self.chunks_cache.keys().next().unwrap();
            self.chunks_cache.remove(&key);
        }

        match self.chunks_cache.entry(chunk_index) {
            std::collections::hash_map::Entry::Occupied(e) => Ok(e.into_mut()),
            std::collections::hash_map::Entry::Vacant(e) => {
                let mut chunk = [0u8; CHUNK_SIZE];
                let read_count = source_kind
                    .read(chunk_index as u64 * CHUNK_SIZE as u64, &mut chunk)
                    .await?;
                if read_count < chunk.len() {
                    // If we read less than page size, it means we reached EOF. Note that tokio read_buf doc
                    // say that it's possible that you get an EOF once and then could get more data from the file.
                    // But I guess this only happen if the file is being written to while you read - which is not
                    // something we want to handle. So we decide that the first EOF we get is the true EOF
                    if let Some(source_len) = self.source_len {
                        return Err(Error::SourceError(format!("Reached EOF a second time (previous source_len={}), now read_count={} at chunk_index={}", source_len, read_count, chunk_index)));
                    } else {
                        self.source_len =
                            Some(chunk_index as u64 * CHUNK_SIZE as u64 + read_count as u64);
                    }
                }
                return Ok(e.insert(chunk));
            }
        }
    }

    pub async fn read_exact(
        &mut self,
        source_kind: &mut SourceKind,
        offset: u64,
        buf: &mut [u8],
    ) -> Result<(), Error> {
        let start_chunk = (offset / CHUNK_SIZE as u64) as u32;
        let end_chunk = ((offset + buf.len() as u64) / CHUNK_SIZE as u64) as u32;
        let mut buf_offset = 0;
        for chunk_id in start_chunk..end_chunk + 1 {
            let chunk_start_offset = chunk_id as i64 * CHUNK_SIZE as i64;
            let chunk = self.read_chunk(source_kind, chunk_id).await?;

            let chunk_from = std::cmp::max(offset as i64 - chunk_start_offset, 0) as usize;
            let chunk_to = std::cmp::min(
                (offset as i64 + buf.len() as i64) - chunk_start_offset,
                CHUNK_SIZE as i64,
            ) as usize;
            let read_count = chunk_to - chunk_from;
            buf[buf_offset..buf_offset + read_count].copy_from_slice(&chunk[chunk_from..chunk_to]);

            // Read past EOF check
            if let Some(source_len) = self.source_len {
                if offset + buf.len() as u64 > source_len {
                    return Err(Error::SourceError(format!(
                        "Trying to read past EOF (source_len={}), offset + buf.len() = {}",
                        source_len,
                        offset as usize + buf.len()
                    )));
                }
            }
            buf_offset += read_count;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Source {
    kind: SourceKind,
    cache: ChunkCache,
}

impl Source {
    fn new(kind: SourceKind) -> Source {
        Source {
            kind,
            cache: ChunkCache::new(),
        }
    }

    pub async fn new_from_source_spec(source_spec: &str) -> Result<Source, Error> {
        let source_string = source_spec.to_string();
        if source_string.starts_with("/vsis3/") {
            let source = Source::new(SourceKind::S3(
                S3Source::new(source_string.strip_prefix("/vsis3/").unwrap()).await?,
            ));
            Ok(source)
        } else if source_string.starts_with("/vsigs/") {
            let source = Source::new(SourceKind::Gcs(
                GCSSource::new(source_string.strip_prefix("/vsigs/").unwrap()).await?,
            ));
            Ok(source)
        } else {
            let source = Source::new(SourceKind::File(FileSource::new(&source_string).await?));
            Ok(source)
        }
    }

    // Read going through the chunk cache
    pub async fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), Error> {
        self.cache.read_exact(&mut self.kind, offset, buf).await
    }

    // Read bypassing the chunk cache
    pub async fn read_exact_direct(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), Error> {
        self.kind.read_exact(offset, buf).await
    }

    pub fn get_stats(&self) -> String {
        self.kind.get_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::SourceKind;
    use crate::errors::Error;
    use crate::sources::{MemorySource, Source};
    use std::fs::File;
    use std::io::Read;

    fn random_buf(buf: &mut [u8]) {
        let mut f = File::open("/dev/urandom").unwrap();
        f.read_exact(buf).unwrap();
    }

    #[tokio::test]
    async fn test_cached_source() {
        for data_len in [100, 1025, 5000] {
            let mut data = vec![0u8; data_len];
            random_buf(&mut data);

            let mut mem_source = Source::new(SourceKind::Memory(MemorySource::new(data.clone())));

            for offset in [0, 50, 1026] {
                if offset > data_len {
                    continue;
                }
                let mut out = vec![0u8; data_len - offset];
                mem_source
                    .read_exact(offset as u64, &mut out)
                    .await
                    .unwrap();
                assert_eq!(out, data[offset..]);
            }
        }
    }

    #[tokio::test]
    async fn test_cached_source_cache_hits() {
        let mut data = vec![0u8; 2000];
        random_buf(&mut data);

        let mut mem_source = Source::new(SourceKind::Memory(MemorySource::new(data.clone())));

        let offset = 513;
        let mut out = vec![0u8; data.len() - offset];
        mem_source
            .read_exact(offset as u64, &mut out)
            .await
            .unwrap();
        mem_source
            .read_exact(offset as u64, &mut out)
            .await
            .unwrap();
        assert_eq!(out, data[offset..]);
        let stats = mem_source.get_stats();

        // The second read should be fully cached
        assert!(stats.contains("read_counts=1"));
    }

    #[tokio::test]
    async fn test_direct_source_cache_hits() {
        let mut data = vec![0u8; 2000];
        random_buf(&mut data);

        let mut mem_source = Source::new(SourceKind::Memory(MemorySource::new(data.clone())));

        let offset = 513;
        let mut out = vec![0u8; data.len() - offset];
        mem_source
            .read_exact_direct(offset as u64, &mut out)
            .await
            .unwrap();
        mem_source
            .read_exact_direct(offset as u64, &mut out)
            .await
            .unwrap();
        assert_eq!(out, data[offset..]);
        let stats = mem_source.get_stats();

        // No cache should be used
        assert!(stats.contains("read_counts=2"));
    }

    #[tokio::test]
    async fn test_cached_source_read_past_eof() {
        let mut data = vec![0u8; 50];
        random_buf(&mut data);

        let mut mem_source = Source::new(SourceKind::Memory(MemorySource::new(data.clone())));

        let mut out = vec![0u8; 10];
        let res = mem_source.read_exact(45, &mut out).await;
        assert!(matches!(res, Err(Error::SourceError(_msg))));
    }
}
