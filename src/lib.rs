mod epsg;
mod errors;
pub mod npy;
mod sources;
mod tiff;
pub mod tiler;

pub use errors::Error;
pub use tiff::cog::{ImageRect, COG};
pub use tiff::ifd::{FullyDecodedIFDEntry, TIFFReader};

pub async fn open(source_spec: &String) -> Result<COG, Error> {
    COG::open(source_spec).await
}
