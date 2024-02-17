use std::{collections::HashMap, fmt, io};

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
    /// See https://docs.rs/tokio/latest/tokio/io/trait.AsyncReadExt.html#method.read
    async fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, io::Error> {
        match self {
            Source::File(s) => s.read(offset, buf).await,
            Source::S3(s) => s.read(offset, buf).await,
            Source::Memory(s) => s.read(offset, buf).await,
        }
    }

    /// Similar to read_exact but with the difference that if EOF is reached, the bytes until EOF are correctly
    /// read into buf
    /// Returns the number of bytes read
    pub async fn read_to_fill_buf_or_eof(
        &mut self,
        offset: u64,
        buf: &mut [u8],
    ) -> Result<usize, io::Error> {
        let mut idx = 0;
        loop {
            match self.read(offset + idx as u64, &mut buf[idx..]).await {
                Ok(n) => {
                    // See the doc for AsyncReadExt.read for the 2 possible cases where 0 can be returned
                    if n == 0 {
                        // EOF or buf[idx..] has a length of 0
                        return Ok(idx);
                    }
                    idx += n;
                }
                Err(e) => return Err(e),
            }
            if idx >= buf.len() - 1 {
                return Ok(idx);
            }
        }
    }
}

const CACHE_PAGE_SIZE: usize = 1024;

pub struct CachedSource {
    source: Source,
    // The key is the page id
    cache: HashMap<u64, [u8; CACHE_PAGE_SIZE]>,
    cache_hits: u64,
    // Once we have reached EOF, we store the source len here
    source_len: Option<usize>,
}

impl fmt::Debug for CachedSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CachedSource")
            .field("source", &self.source)
            .field("cache_hits", &self.cache_hits)
            .finish()
    }
}

impl CachedSource {
    pub fn new(source: Source) -> Self {
        CachedSource {
            source,
            cache: HashMap::new(),
            cache_hits: 0,
            source_len: None,
        }
    }

    fn cache_hits_count(&self) -> u64 {
        self.cache_hits
    }

    fn cached_pages_count(&self) -> usize {
        self.cache.len()
    }

    pub fn cache_stats(&self) -> String {
        format!(
            "cache_hits={}, cached_pages={}",
            self.cache_hits_count(),
            self.cached_pages_count()
        )
    }

    async fn read_page(&mut self, page_id: u64) -> Result<&[u8; CACHE_PAGE_SIZE], Error> {
        match self.cache.entry(page_id) {
            std::collections::hash_map::Entry::Vacant(e) => {
                let mut page = [0u8; CACHE_PAGE_SIZE];
                let read_count = self
                    .source
                    .read_to_fill_buf_or_eof(page_id * CACHE_PAGE_SIZE as u64, &mut page)
                    .await?;
                if read_count < page.len() {
                    // If we read less than page size, it means we reached EOF. Note that tokio read_buf doc
                    // say that it's possible that you get an EOF once and then could get more data from the file.
                    // But I guess this only happen if the file is being written to while you read - which is not
                    // something we want to handle. So we decide that the first EOF we get is the true EOF
                    if let Some(source_len) = self.source_len {
                        return Err(Error::SourceError(format!("Reached EOF a second time (previous source_len={}), now read_count={} at page_id={}", source_len, read_count, page_id)));
                    } else {
                        self.source_len = Some(page_id as usize * CACHE_PAGE_SIZE + read_count);
                    }
                }
                return Ok(e.insert(page));
            }
            std::collections::hash_map::Entry::Occupied(e) => {
                self.cache_hits += 1;
                return Ok(e.into_mut());
            }
        }
    }

    pub async fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), Error> {
        let start_page = offset / CACHE_PAGE_SIZE as u64;
        let end_page = (offset + buf.len() as u64) / CACHE_PAGE_SIZE as u64;
        let mut buf_offset = 0;
        for page_id in start_page..end_page + 1 {
            let page_start_offset = page_id as i64 * CACHE_PAGE_SIZE as i64;
            let page = self.read_page(page_id).await?;

            let page_from = std::cmp::max(offset as i64 - page_start_offset, 0) as usize;
            let page_to = std::cmp::min(
                (offset as i64 + buf.len() as i64) - page_start_offset,
                CACHE_PAGE_SIZE as i64,
            ) as usize;
            let read_count = page_to - page_from;
            buf[buf_offset..buf_offset + read_count].copy_from_slice(&page[page_from..page_to]);

            // Read past EOF check
            if let Some(source_len) = self.source_len {
                if offset as usize + buf.len() > source_len {
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

#[cfg(test)]
mod tests {
    use super::{CachedSource, MemorySource, Source};
    use crate::errors::Error;
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

            let mem_source = MemorySource::new(data.clone());
            let mut cached_source = CachedSource::new(Source::Memory(mem_source));

            for offset in [0, 50, 1026] {
                if offset > data_len {
                    continue;
                }
                let mut out = vec![0u8; data_len - offset];
                cached_source
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

        let mem_source = MemorySource::new(data.clone());
        let mut cached_source = CachedSource::new(Source::Memory(mem_source));

        let offset = 513;
        let mut out = vec![0u8; data.len() - offset];
        cached_source
            .read_exact(offset as u64, &mut out)
            .await
            .unwrap();
        cached_source
            .read_exact(offset as u64, &mut out)
            .await
            .unwrap();
        assert_eq!(out, data[offset..]);
        // The second read should be fully cached
        assert_eq!(cached_source.cache_hits_count(), 2);
        assert_eq!(cached_source.cached_pages_count(), 2);
    }

    #[tokio::test]
    async fn test_cached_source_read_past_eof() {
        let mut data = vec![0u8; 50];
        random_buf(&mut data);

        let mem_source = MemorySource::new(data.clone());
        let mut cached_source = CachedSource::new(Source::Memory(mem_source));

        let mut out = vec![0u8; 10];
        let res = cached_source.read_exact(45, &mut out).await;
        assert!(matches!(res, Err(Error::SourceError(_msg))));
    }
}
