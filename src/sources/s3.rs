use std::cmp::min;

use crate::errors::Error;
use bytes::Buf;
use reqwest::Client;

pub struct S3Source {
    client: Client,
    blob_name: String,
    // TODO: Add stats about requests ?
}

impl S3Source {
    pub async fn new(filename: &str) -> Result<S3Source, Error> {
        let client = Client::builder().build()?;
        Ok(S3Source {
            client,
            blob_name: filename.to_string(),
        })
    }

    /// See https://docs.rs/tokio/latest/tokio/io/trait.AsyncReadExt.html#method.read_exact
    pub async fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Error> {
        // TODO: Take endpoint from env var
        let url = format!("http://localhost:9000/{}", self.blob_name);
        let do_request = |from: u64, to: u64| {
            println!("Range request: {}-{}", from, to);
            self.client
                .get(url.clone())
                .header("Range", format!("bytes={}-{}", from, to))
                .send()
        };
        let mut body = {
            let resp = do_request(offset, offset + buf.len() as u64).await?;
            // We check for explicit 206 (Partial Content) because if the server would not support range requests,
            // it could just reply with 200 and the whole document, but we don't support/want this
            // here
            if resp.status().as_u16() == 206 {
                resp.bytes().await?
            } else {
                return Err(Error::OtherError(format!(
                    "Request failed, code={}: {}",
                    resp.status().as_u16(),
                    resp.text().await?,
                )));
            }
        };

        // let mut body = resp.bytes().await?;
        let body_len = body.remaining();
        // We need to handle incomplete response because copy_to_slice will panic if buf.remaining() < buf.len()
        if body.remaining() < buf.len() {
            // TODO: Should we raise an error or return incomplete read ? Incomplete read seem to make sense
        }
        let len_to_copy = min(body_len, buf.len());
        body.copy_to_slice(&mut buf[0..len_to_copy]);
        Ok(len_to_copy)
    }
}

#[cfg(test)]
mod tests {
    use crate as acog;

    /// These tests require minio running with the setup from the `docker-compose.yml` file
    #[tokio::test]
    async fn test_minio_cog_info_example_1() {
        let cog = acog::open("/vsis3/public/example_1_cog_3857_nocompress.tif")
            .await
            .unwrap();
        assert_eq!(cog.width(), 370);
        assert_eq!(cog.height(), 276);
        assert_eq!(cog.nbands(), 4);
        assert_eq!(cog.overviews.len(), 1);
    }
}
