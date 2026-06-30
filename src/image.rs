/// Decoded RGBA pixel data from an SVG render operation.
///
/// Pixels are stored in premultiplied alpha, RGBA8888 format
/// (4 bytes per pixel, row-major order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageData {
    /// Width of the image in pixels.
    pub width: u32,
    /// Height of the image in pixels.
    pub height: u32,
    /// Number of bytes per row (`width × 4`).
    pub row_bytes: usize,
    /// Flat pixel buffer in RGBA8888 premultiplied format.
    pub rgba: Vec<u8>,
}
