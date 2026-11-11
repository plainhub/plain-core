use core::fmt;
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkType(pub [u8; 4]);
pub const IHDR: ChunkType = ChunkType(*b"IHDR");
pub const PLTE: ChunkType = ChunkType(*b"PLTE");
pub const IDAT: ChunkType = ChunkType(*b"IDAT");
pub const IEND: ChunkType = ChunkType(*b"IEND");
pub const tRNS: ChunkType = ChunkType(*b"tRNS");
pub const bKGD: ChunkType = ChunkType(*b"bKGD");
pub const tIME: ChunkType = ChunkType(*b"tIME");
pub const pHYs: ChunkType = ChunkType(*b"pHYs");
pub const cHRM: ChunkType = ChunkType(*b"cHRM");
pub const gAMA: ChunkType = ChunkType(*b"gAMA");
pub const sRGB: ChunkType = ChunkType(*b"sRGB");
pub const iCCP: ChunkType = ChunkType(*b"iCCP");
pub const cICP: ChunkType = ChunkType(*b"cICP");
pub const mDCV: ChunkType = ChunkType(*b"mDCV");
pub const cLLI: ChunkType = ChunkType(*b"cLLI");
pub const eXIf: ChunkType = ChunkType(*b"eXIf");
pub const tEXt: ChunkType = ChunkType(*b"tEXt");
pub const zTXt: ChunkType = ChunkType(*b"zTXt");
pub const iTXt: ChunkType = ChunkType(*b"iTXt");
pub const sBIT: ChunkType = ChunkType(*b"sBIT");
pub const acTL: ChunkType = ChunkType(*b"acTL");
pub const fcTL: ChunkType = ChunkType(*b"fcTL");
pub const fdAT: ChunkType = ChunkType(*b"fdAT");
pub fn is_critical(ChunkType(type_): ChunkType) -> bool {
    type_[0] & 32 == 0
}
pub fn is_private(ChunkType(type_): ChunkType) -> bool {
    type_[1] & 32 != 0
}
pub fn reserved_set(ChunkType(type_): ChunkType) -> bool {
    type_[2] & 32 != 0
}
pub fn safe_to_copy(ChunkType(type_): ChunkType) -> bool {
    type_[3] & 32 != 0
}
impl fmt::Debug for ChunkType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct DebugType([u8; 4]);
        impl fmt::Debug for DebugType {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                for &c in &self.0[..] {
                    write!(f, "{}", char::from(c).escape_debug())?;
                }
                Ok(())
            }
        }
        f.debug_struct("ChunkType")
            .field("type", &DebugType(self.0))
            .field("critical", &is_critical(*self))
            .field("private", &is_private(*self))
            .field("reserved", &reserved_set(*self))
            .field("safecopy", &safe_to_copy(*self))
            .finish()
    }
}
