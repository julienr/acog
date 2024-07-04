mod bbox;
mod epsg;
mod errors;
pub mod image;
mod math;
pub mod npy;
pub mod ppm;
mod sources;
mod tiff;
pub mod tiler;

pub use errors::Error;
pub use tiff::cog::{ImageRect, COG};
pub use tiff::ifd::{FullyDecodedIFDEntry, TIFFReader};

pub async fn open(source_spec: &str) -> Result<COG, Error> {
    COG::open(source_spec).await
}
