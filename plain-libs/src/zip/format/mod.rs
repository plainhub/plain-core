pub mod aes;
pub mod blocks;
pub mod compression;
pub mod data_descriptor;
pub mod extra_fields;
pub(crate) mod find_central_directory;
pub mod flags;
pub(crate) mod functions;
pub mod magic;
pub mod system;
pub mod ffi {
    pub const S_IFREG: u32 = 0b1000_0000_0000_0000;
    pub const S_IFDIR: u32 = 0b0100_0000_0000_0000;
    pub const S_IFLNK: u32 = 0b1010_0000_0000_0000;
}
pub(crate) use find_central_directory::MagicPositionCache;
pub(crate) use find_central_directory::find_central_directory_end;
pub const ZIP64_BYTES_THR: u64 = u32::MAX as u64;
pub const ZIP64_BYTES_THR_U32: u32 = u32::MAX;
pub const ZIP64_ENTRY_THR: usize = u16::MAX as usize;
pub const MIN_VERSION: u8 = 10;
pub const DEFAULT_VERSION: u8 = 45;
