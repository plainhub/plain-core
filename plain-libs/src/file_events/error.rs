use std::error::Error as StdError;
use std::path::PathBuf;
use std::result::Result as StdResult;
use std::{self, fmt, io};

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug)]
pub enum ErrorKind {
    Generic(String),

    Io(io::Error),

    PathNotFound,

    WatchNotFound,

    MaxFilesWatch,
}

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,

    pub paths: Vec<PathBuf>,
}

impl Error {
    pub fn add_path(mut self, path: PathBuf) -> Self {
        self.paths.push(path);
        self
    }

    pub fn set_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.paths = paths;
        self
    }

    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            paths: Vec::new(),
        }
    }

    pub fn generic(msg: &str) -> Self {
        Self::new(ErrorKind::Generic(msg.into()))
    }

    pub fn io(err: io::Error) -> Self {
        Self::new(ErrorKind::Io(err))
    }

    pub fn io_watch(err: io::Error) -> Self {
        if err.kind() == io::ErrorKind::NotFound {
            Self::path_not_found()
        } else {
            Self::io(err)
        }
    }

    pub fn path_not_found() -> Self {
        Self::new(ErrorKind::PathNotFound)
    }

    pub fn watch_not_found() -> Self {
        Self::new(ErrorKind::WatchNotFound)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let error = match self.kind {
            ErrorKind::PathNotFound => "No path was found.".into(),
            ErrorKind::WatchNotFound => "No watch was found.".into(),

            ErrorKind::Generic(ref err) => err.clone(),
            ErrorKind::Io(ref err) => err.to_string(),
            ErrorKind::MaxFilesWatch => "OS file watch limit reached.".into(),
        };

        if self.paths.is_empty() {
            write!(f, "{}", error)
        } else {
            write!(f, "{} about {:?}", error, self.paths)
        }
    }
}

impl StdError for Error {
    fn cause(&self) -> Option<&dyn StdError> {
        match self.kind {
            ErrorKind::Io(ref cause) => Some(cause),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::io(err)
    }
}

impl<T> From<std::sync::mpsc::SendError<T>> for Error {
    fn from(err: std::sync::mpsc::SendError<T>) -> Self {
        Error::generic(&format!("internal channel disconnect: {:?}", err))
    }
}

impl From<std::sync::mpsc::RecvError> for Error {
    fn from(err: std::sync::mpsc::RecvError) -> Self {
        Error::generic(&format!("internal channel disconnect: {:?}", err))
    }
}

impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        Error::generic(&format!("internal mutex poisoned: {:?}", err))
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/file_events/error.rs"]
mod tests;
