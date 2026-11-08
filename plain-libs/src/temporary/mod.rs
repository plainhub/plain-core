// Temporary file creation and persistence adapted from tempfile 3.27.0 (SOURCE.json).
use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    fmt,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

pub fn tempdir() -> io::Result<TempDir> {
    Builder::new().tempdir_in(std::env::temp_dir())
}

pub fn tempfile() -> io::Result<File> {
    tempfile_in(std::env::temp_dir())
}

pub fn tempfile_in(dir: impl AsRef<Path>) -> io::Result<File> {
    platform::anonymous(dir.as_ref())
}

#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn keep(mut self) -> PathBuf {
        std::mem::take(&mut self.path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if !self.path.as_os_str().is_empty() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[derive(Debug)]
pub struct NamedTempFile {
    path: FilePath,
    file: File,
}

#[derive(Debug)]
struct FilePath(PathBuf);

impl Drop for FilePath {
    fn drop(&mut self) {
        if !self.0.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.0);
        }
    }
}

impl NamedTempFile {
    pub fn new() -> io::Result<Self> {
        Self::new_in(std::env::temp_dir())
    }

    pub fn new_in(dir: impl AsRef<Path>) -> io::Result<Self> {
        Builder::new().tempfile_in(dir)
    }

    pub fn path(&self) -> &Path {
        &self.path.0
    }

    pub fn as_file(&self) -> &File {
        &self.file
    }

    pub fn as_file_mut(&mut self) -> &mut File {
        &mut self.file
    }

    pub fn reopen(&self) -> io::Result<File> {
        platform::reopen(&self.file, &self.path.0)
    }

    pub fn persist(self, destination: impl AsRef<Path>) -> Result<File, PersistError> {
        self.persist_to(destination.as_ref(), true)
    }

    pub fn persist_noclobber(self, destination: impl AsRef<Path>) -> Result<File, PersistError> {
        self.persist_to(destination.as_ref(), false)
    }

    fn persist_to(mut self, destination: &Path, overwrite: bool) -> Result<File, PersistError> {
        match platform::persist(&self.path.0, destination, overwrite) {
            Ok(()) => {
                self.path.0.clear();
                // The file is still open after the name has moved.
                Ok(self.file)
            }
            Err(error) => Err(PersistError { error, file: self }),
        }
    }
}

impl Write for NamedTempFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.file.write_all(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[derive(Debug)]
pub struct PersistError {
    pub error: io::Error,
    pub file: NamedTempFile,
}

impl fmt::Display for PersistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for PersistError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

pub struct Builder<'a> {
    prefix: &'a OsStr,
    suffix: &'a OsStr,
}

impl<'a> Builder<'a> {
    pub fn new() -> Self {
        Self {
            prefix: OsStr::new(".tmp"),
            suffix: OsStr::new(""),
        }
    }

    pub fn prefix(&mut self, value: &'a (impl AsRef<OsStr> + ?Sized)) -> &mut Self {
        self.prefix = value.as_ref();
        self
    }

    pub fn suffix(&mut self, value: &'a (impl AsRef<OsStr> + ?Sized)) -> &mut Self {
        self.suffix = value.as_ref();
        self
    }

    pub fn tempdir(&self) -> io::Result<TempDir> {
        self.tempdir_in(std::env::temp_dir())
    }

    pub fn tempdir_in(&self, dir: impl AsRef<Path>) -> io::Result<TempDir> {
        let mut builder = DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        self.create(dir.as_ref(), |path| {
            builder.create(&path)?;
            Ok(TempDir { path })
        })
    }

    pub fn tempfile_in(&self, dir: impl AsRef<Path>) -> io::Result<NamedTempFile> {
        self.create(dir.as_ref(), |path| {
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            let file = platform::open_named(&path, &mut options)?;
            Ok(NamedTempFile {
                file,
                path: FilePath(path),
            })
        })
    }

    fn create<T>(
        &self,
        dir: &Path,
        mut open: impl FnMut(PathBuf) -> io::Result<T>,
    ) -> io::Result<T> {
        let dir = if dir.is_absolute() {
            Cow::Borrowed(dir)
        } else {
            Cow::Owned(std::env::current_dir()?.join(dir))
        };
        const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
        let mut name = OsString::with_capacity(self.prefix.len() + 6 + self.suffix.len());
        for _ in 0..65_536 {
            let mut random = rand::random::<u64>();
            let mut digits = [0; 6];
            for digit in &mut digits {
                *digit = ALPHABET[(random % 62) as usize];
                random /= 62;
            }
            name.clear();
            name.push(self.prefix);
            name.push(str::from_utf8(&digits).unwrap());
            name.push(self.suffix);
            match open(dir.join(&name)) {
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                result => return result,
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "too many temporary files exist",
        ))
    }
}

impl From<PersistError> for io::Error {
    fn from(value: PersistError) -> Self {
        value.error
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/temporary/mod.rs"]
mod tests;
