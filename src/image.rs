pub struct ImageBuffer {
    pub width: usize,
    pub height: usize,
    pub nbands: usize,
    // The image data stored in row-major order
    pub data: Vec<u8>,
}
