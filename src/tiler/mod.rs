use crate::Error;
use crate::COG;

/// XYZ tile coordinates
#[derive(Debug, Clone, Copy)]
pub struct TileCoords {
    pub x: u64,
    pub y: u64,
    pub z: u64,
}

pub fn extract_tile(cog: COG, coords: TileCoords) -> Result<Vec<u8>, TileCoords> {
    todo!()
}
