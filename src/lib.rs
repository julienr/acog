mod bbox;
mod epsg;
mod errors;
mod hex;
pub mod image;
mod math;
pub mod ppm;
mod sources;
mod tiff;
pub mod tiler;

pub use bbox::BoundingBox;
pub use errors::Error;
pub use tiff::cog::{ImageRect, COG};
pub use tiff::data_types::DataType;
pub use tiff::ifd::{FullyDecodedIFDEntry, TIFFReader};

pub async fn open(source_spec: &str) -> Result<COG, Error> {
    COG::open(source_spec).await
}
