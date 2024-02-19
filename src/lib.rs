mod epsg;
mod errors;
pub mod npy;
mod sources;
mod tiff;

pub use errors::Error;
pub use tiff::cog::COG;
pub use tiff::ifd::{FullyDecodedIFDEntry, TIFFReader};

pub async fn open(source_spec: &String) -> Result<COG, Error> {
    COG::open(source_spec).await
}
