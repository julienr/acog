mod errors;
pub mod npy;
mod sources;
mod tiff;

pub use errors::Error;
pub use tiff::ifd::FullyDecodedIFDEntry;
pub use tiff::ifd::TIFFReader;

pub async fn open(source_spec: &str) -> Result<TIFFReader, Error> {
    let source_string = source_spec.to_string();
    if source_string.starts_with("/vsis3/") {
        let file_source =
            sources::S3Source::new(source_string.strip_prefix("/vsis3/").unwrap()).await?;
        let reader = TIFFReader::open(sources::Source::S3(file_source)).await?;
        Ok(reader)
    } else {
        let file_source = sources::FileSource::new(&source_string).await?;
        let reader = TIFFReader::open(sources::Source::File(file_source)).await?;
        Ok(reader)
    }
}
