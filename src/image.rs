#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub row_bytes: usize,
    pub rgba: Vec<u8>,
}
