mod errors;
pub mod npy;
mod tiff;

pub use errors::Error;
pub use tiff::ifd::TIFFReader;
