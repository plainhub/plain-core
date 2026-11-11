#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy)]
pub struct Config {
    pub archive_offset: ArchiveOffset,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchiveOffset {
    #[default]
    Detect,
    Known(u64),
}
