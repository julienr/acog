// https://docs.aws.amazon.com/IAM/latest/UserGuide/create-signed-request.html
// Alternative rust implementation for inspiration
// https://github.com/uv-rust/s3v4/blob/main/src/lib.rs
// Also AWS-provided examples
// https://github.com/aws-samples/sigv4-signing-examples/blob/main/no-sdk/python/main.py

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::result::Result;

use crate::hex::bytes_to_hex_string;
use crate::Error;

const FMT_YYYYMMDD_HHMMSS: &str = "%Y%m%dT%H%M%SZ";
const FMT_YYYYMMDD: &str = "%Y%m%d";

fn canonical_request(method: &str, uri: &str, host: &str) -> String {
    let http_method = method.to_uppercase();
    let canonical_uri: String = uri.to_string();
    let canonical_query_string: String = "".to_string();
    let canonical_headers = format!("host:{host}\n");
    let signed_headers = "host".to_string();
    let hashed_payload = bytes_to_hex_string(&Sha256::digest("".to_string().as_bytes()));

    [
        http_method,
        canonical_uri,
        canonical_query_string,
        canonical_headers,
        signed_headers,
        hashed_payload,
    ]
    .join("\n")
}

fn scope(timestamp: &DateTime<Utc>, region: &str) -> String {
    let datetime = timestamp.format(FMT_YYYYMMDD).to_string();
    format!("{datetime}/{region}/s3/aws4_request")
}

fn string_to_sign(
    timestamp: &DateTime<Utc>,
    method: &str,
    uri: &str,
    host: &str,
    region: &str,
) -> String {
    let canonical_request = canonical_request(method, uri, host);
    let hashed_canonical_request =
        bytes_to_hex_string(&Sha256::digest(canonical_request.as_bytes()));
    let request_date_time = timestamp.format(FMT_YYYYMMDD_HHMMSS).to_string();
    [
        "AWS4-HMAC-SHA256",
        &request_date_time,
        &scope(timestamp, region),
        &hashed_canonical_request,
    ]
    .join("\n")
}

fn hmac(key: &[u8], value: &str) -> Vec<u8> {
    // TODO: Remove unwrap
    let mut h = Hmac::<Sha256>::new_from_slice(key).unwrap();
    h.update(value.as_bytes());
    h.finalize().into_bytes().to_vec()
}

fn signing_key(timestamp: &DateTime<Utc>, secret_key: &str, region: &str) -> Vec<u8> {
    let date_key = hmac(
        format!("AWS4{secret_key}").as_bytes(),
        &timestamp.format(FMT_YYYYMMDD).to_string(),
    );
    let date_region_key = hmac(&date_key, region);
    let date_region_service_key = hmac(&date_region_key, "s3");
    hmac(&date_region_service_key, "aws4_request")
}

fn compute_signature(
    method: &str,
    host: &str,
    uri: &str,
    region: &str,
    timestamp: &DateTime<Utc>,
    secret_key: &str,
) -> Result<String, Error> {
    let to_sign = string_to_sign(timestamp, method, uri, host, region);
    let key = signing_key(timestamp, secret_key, region);
    let signature = bytes_to_hex_string(&hmac(&key, &to_sign));
    Ok(signature)
}

fn compute_signature_headers(
    method: &str,
    host: &str,
    uri: &str,
    region: &str,
    timestamp: &DateTime<Utc>,
    access_key: &str,
    secret_key: &str,
) -> Result<SignatureHeaders, Error> {
    let signature = compute_signature(method, host, uri, region, timestamp, secret_key)?;
    let datestamp = timestamp.format(FMT_YYYYMMDD).to_string();
    let scope = format!("{datestamp}/{region}/s3/aws4_request");
    let authorization_header = format!("AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders=host, Signature={signature}");

    let headers = SignatureHeaders {
        host_header: host.to_string(),
        amz_date_header: timestamp.format(FMT_YYYYMMDD_HHMMSS).to_string(),
        authorization_header,
    };
    Ok(headers)
}

// The result of signing a request is a series of headers that should be added to the request
pub struct SignatureHeaders {
    // The 'Host' header
    pub host_header: String,
    // The 'x-amz-date' header
    pub amz_date_header: String,
    // The 'Authorization' header
    pub authorization_header: String,
}

// Those are the ones used by minio for localdev / tests
const MINIO_DEFAULT_ACCESS_KEY: &str = "V5NSAQUNLNZ5AP7VLLS6";
const MINIO_DEFAULT_SECRET_KEY: &str = "bu0K3n0kEag8GKfckKPBg4Vu8O8EuYu2UO/wNfqI";

pub fn sign_request(method: &str, host: &str, uri: &str) -> Result<SignatureHeaders, Error> {
    let secret_key =
        std::env::var("AWS_SECRET_ACCESS_KEY").unwrap_or(MINIO_DEFAULT_SECRET_KEY.to_string());
    let access_key =
        std::env::var("AWS_ACCESS_KEY_ID").unwrap_or(MINIO_DEFAULT_ACCESS_KEY.to_string());
    let region = std::env::var("AWS_DEFAULT_REGION").unwrap_or("us-east-1".to_string());
    let timestamp = Utc::now();
    compute_signature_headers(
        method,
        host,
        uri,
        &region,
        &timestamp,
        &access_key,
        &secret_key,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_request, compute_signature, compute_signature_headers, signing_key,
        string_to_sign,
    };
    use chrono::{NaiveDate, TimeZone, Utc};

    // Test cases below are generated by adapting this example script to print intermediate values
    // See `misc/aws_sign.py`
    // https://github.com/aws-samples/sigv4-signing-examples/blob/main/no-sdk/python/main.py

    #[test]
    fn test_canonical_request() {
        let actual =
            canonical_request("GET", "/public/example_1_cog_deflate.tif", "localhost:9000");
        // Generated by misc/aws_sign.py
        let expected = "GET\n/public/example_1_cog_deflate.tif\n\nhost:localhost:9000\n\nhost\ne3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_string_to_sign() {
        let t = Utc.from_utc_datetime(&NaiveDate::from_ymd_opt(2024, 9, 28).unwrap().into());
        let actual = string_to_sign(
            &t,
            "GET",
            "/public/example_1_cog_deflate.tif",
            "localhost:9000",
            "us-east-1",
        );
        // Generated by misc/aws_sign.py - make sure you hardcode the same date as above
        let expected = "AWS4-HMAC-SHA256\n20240928T000000Z\n20240928/us-east-1/s3/aws4_request\nc32076749fe36e2e6324aa0d37ef72c39f169b442d05503d09c2a5c9131ea9d3";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_signing_key() {
        let t = Utc.from_utc_datetime(&NaiveDate::from_ymd_opt(2024, 9, 28).unwrap().into());
        let secret_key = "bu0K3n0kEag8GKfckKPBg4Vu8O8EuYu2UO/wNfqI";
        let actual = signing_key(&t, secret_key, "us-east-1");
        // Generated by misc/aws_sign.py - make sure you hardcode the same date as above
        let expected = b"y\xf1\xf1Ve=\xfa\xd6;\x90\xff}\xd2m\xdd\xbd\xf3\xdfd\x8b\x03\xecc\x0e\xaa\xc9\"(3\xaf\x0f\xf7";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_compute_signature() {
        let t = Utc.from_utc_datetime(&NaiveDate::from_ymd_opt(2024, 9, 28).unwrap().into());
        let secret_key = "bu0K3n0kEag8GKfckKPBg4Vu8O8EuYu2UO/wNfqI";
        let actual = compute_signature(
            "GET",
            "localhost:9000",
            "/public/example_1_cog_deflate.tif",
            "us-east-1",
            &t,
            &secret_key,
        )
        .unwrap();
        // Generated by misc/aws_sign.py - make sure you hardcode the same date as above
        let expected = "4183485cce9a6183907a33af3dc89872f944691577926b69616fb4a4623e1212";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_compute_signature_headers() {
        let t = Utc.from_utc_datetime(&NaiveDate::from_ymd_opt(2024, 9, 28).unwrap().into());
        let access_key = "V5NSAQUNLNZ5AP7VLLS6";
        let secret_key = "bu0K3n0kEag8GKfckKPBg4Vu8O8EuYu2UO/wNfqI";
        let actual = compute_signature_headers(
            "GET",
            "localhost:9000",
            "/public/example_1_cog_deflate.tif",
            "us-east-1",
            &t,
            &access_key,
            &secret_key,
        )
        .unwrap();
        // Generated by misc/aws_sign.py - make sure you hardcode the same date as above
        let expected_authorization = "AWS4-HMAC-SHA256 Credential=V5NSAQUNLNZ5AP7VLLS6/20240928/us-east-1/s3/aws4_request, SignedHeaders=host, Signature=4183485cce9a6183907a33af3dc89872f944691577926b69616fb4a4623e1212";
        assert_eq!(actual.host_header, "localhost:9000");
        assert_eq!(actual.amz_date_header, "20240928T000000Z");
        assert_eq!(actual.authorization_header, expected_authorization);
    }
}
