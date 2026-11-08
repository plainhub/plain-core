// Adapted from notify-types 1.0.1 (notify-types/SOURCE.json).
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessMode {
    Any,
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessKind {
    Open(AccessMode),
    Close(AccessMode),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateKind {
    Any,
    File,
    Folder,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataChange {
    Any,
    Size,
    Content,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataKind {
    Any,
    Ownership,
    Extended,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenameMode {
    Any,
    To,
    From,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModifyKind {
    Any,
    Data(DataChange),
    Metadata(MetadataKind),
    Name(RenameMode),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoveKind {
    Any,
    File,
    Folder,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventKind {
    Access(AccessKind),
    Create(CreateKind),
    Modify(ModifyKind),
    Remove(RemoveKind),
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub kind: EventKind,
    pub paths: Vec<PathBuf>,
    rescan: bool,
}

impl Event {
    pub fn new(kind: EventKind) -> Self {
        Self {
            kind,
            paths: Vec::new(),
            rescan: false,
        }
    }

    pub fn need_rescan(&self) -> bool {
        self.rescan
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        windows
    ))]
    pub(super) fn rescan() -> Self {
        Self {
            rescan: true,
            ..Self::new(EventKind::Other)
        }
    }

    pub fn add_path(mut self, path: PathBuf) -> Self {
        self.paths.push(path);
        self
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub(super) fn add_some_path(self, path: Option<PathBuf>) -> Self {
        if let Some(path) = path {
            self.add_path(path)
        } else {
            self
        }
    }
}
