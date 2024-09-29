use std::cmp::min;

use super::auth::gcs::GCSAuth;
use crate::errors::Error;
use bytes::Buf;
use percent_encoding::{utf8_percent_encode, AsciiSet};
use reqwest::{Client, Response};

const GCS_ENDPOINT: &str = "https://storage.googleapis.com/storage/v1";

// TODO: Should include all the ones described here:
// https://cloud.google.com/storage/docs/request-endpoints#encoding
const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &percent_encoding::CONTROLS.add(b'/').add(b'%');

#[derive(Debug, Default)]
struct Stats {
    requests_count: usize,
}

// https://cloud.google.com/storage/docs/json_api/v1/objects/get?hl=en
fn make_url_for_object(bucket_name: &str, blob_name: &str) -> String {
    let encoded_blob_name: String =
        utf8_percent_encode(blob_name, PATH_SEGMENT_ENCODE_SET).to_string();
    format!(
        "{}/b/{}/o/{}?alt=media",
        GCS_ENDPOINT, bucket_name, encoded_blob_name
    )
}

pub struct GCSSource {
    client: Client,
    auth: GCSAuth,
    bucket_name: String,
    blob_name: String,
    stats: Stats,
}

impl GCSSource {
    pub async fn new(filename: &str) -> Result<GCSSource, Error> {
        let client = Client::builder().build()?;
        let auth = GCSAuth::new()?;
        let (bucket_name, blob_name) = {
            let slash_index = match filename.find('/') {
                Some(v) => v,
                None => {
                    return Err(Error::OtherError(format!(
                        "Failed to extract bucket_name from {}",
                        filename
                    )))
                }
            };
            let splits = filename.split_at(slash_index);
            (splits.0, splits.1.strip_prefix('/').unwrap())
        };
        Ok(GCSSource {
            client,
            auth,
            bucket_name: bucket_name.to_string(),
            blob_name: blob_name.to_string(),
            stats: Default::default(),
        })
    }

    async fn do_request(&mut self, url: &str, from: u64, to: u64) -> Result<Response, Error> {
        let access_token = self.auth.get_access_token(&self.client).await?;
        self.stats.requests_count += 1;
        Ok(self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", access_token.token))
            .header("Range", format!("bytes={}-{}", from, to))
            .send()
            .await?)
    }

    pub async fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Error> {
        let url = make_url_for_object(&self.bucket_name, &self.blob_name);
        let mut body = {
            let resp = self
                .do_request(&url, offset, offset + buf.len() as u64)
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
