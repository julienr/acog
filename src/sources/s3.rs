use std::cmp::min;

use super::auth::aws::sign_request;
use crate::errors::Error;
use bytes::Buf;
use reqwest::{Client, Response};

#[derive(Debug, Default)]
struct Stats {
    requests_count: usize,
}

pub struct S3Source {
    client: Client,
    blob_name: String,
    stats: Stats,
}

impl S3Source {
    pub async fn new(filename: &str) -> Result<S3Source, Error> {
        let client = Client::builder().build()?;
        Ok(S3Source {
            client,
            blob_name: filename.to_string(),
            stats: Default::default(),
        })
    }

    async fn do_request(
        &mut self,
        host: &str,
        uri: &str,
        from: u64,
        to: u64,
    ) -> Result<Response, Error> {
        let headers = sign_request("GET", host, uri)?;
        let url = format!("http://{host}{uri}");
        self.stats.requests_count += 1;
        let req = self
            .client
            .get(url)
            .header("Authorization", headers.authorization_header)
            .header("Range", format!("bytes={}-{}", from, to))
            .header("Host", headers.host_header)
            .header("x-amz-date", headers.amz_date_header);
        Ok(req.send().await?)
    }

    pub async fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Error> {
        // TODO: Take endpoint from env var
        let host = "localhost:9000";
        let uri = format!("/{}", self.blob_name);
        //let url = format!("http://localhost:9000/{}", self.blob_name);
        let mut body = {
            let resp = self
                .do_request(host, &uri, offset, offset + buf.len() as u64)
                .await?;
            // We check for explicit 206 (Partial Content) because if the server would not support range requests,
            // it could just reply with 200 and the whole document, but we don't support/want this
            // here
            if resp.status().as_u16() == 206 {
                // Note that EOF is implicitely handled here because if we do a partial past EOF read, we'll
                // still get a 206 but we can parse the "Content-Range" header to get file size. E.g.:
                //
                //    curl -v -r 558379745-558379761 http://localhost:9000/public/local/marina_cog_nocompress_3857.tif
                //    ...
                //    Content-Range: bytes 558379745-558379749/558379750
                //
                // But this is not necessary, because the server will just return the data until EOF, so
                // our logic below transparently handles this
                //
                // Note that if you do a completely invalid read (both start and end past EOF), then most server
                // will rightly response with a 416 - but that's a sign of a logic error here, so we do
                // error out in this case
                resp.bytes().await?
            } else {
                return Err(Error::OtherError(format!(
                    "Request failed, code={}: {}",
                    resp.status().as_u16(),
                    resp.text().await?,
                )));
            }
        };

        let body_len = body.remaining();
        let len_to_copy = min(body_len, buf.len());
        body.copy_to_slice(&mut buf[0..len_to_copy]);
        Ok(len_to_copy)
    }

    pub fn get_stats(&self) -> String {
        format!("{:?}", self.stats)
    }
}

#[cfg(test)]
mod tests {
    use crate as acog;

    /// There is also a tiler "integration test" in `test_extract_tile_minio`
    ///
    /// These tests require minio running with the setup from the `docker-compose.yml` file
    #[tokio::test]
    async fn test_minio_cog_info_example_1() {
        let cog = acog::open("/vsis3/public/example_1_cog_3857_nocompress.tif")
            .await
            .unwrap();
        assert_eq!(cog.width(), 370);
        assert_eq!(cog.height(), 276);
        assert_eq!(cog.visual_bands_count(), 3);
        assert_eq!(cog.overviews.len(), 1);
    }
}
