use std::vec::Vec;
#[non_exhaustive]
pub enum DecodingResult {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
}
impl DecodingResult {
    pub fn u8(self) -> Option<Vec<u8>> {
        match self {
            DecodingResult::U8(data) => Some(data),
            _ => None,
        }
    }
    pub fn u16(self) -> Option<Vec<u16>> {
        match self {
            DecodingResult::U16(data) => Some(data),
            _ => None,
        }
    }
}
