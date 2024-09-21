// Authentication utilities to get access token to various storage services (e.g. GCS, S3)
pub mod aws;
#[cfg(feature = "gcs")]
pub mod gcs;
