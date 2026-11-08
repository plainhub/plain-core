use super::Builder;
use crate::platform::rename::no_replace;
use std::{
    fs::{self, File, OpenOptions, hard_link},
    io,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    sync::atomic::{AtomicBool, Ordering::Relaxed},
};

pub(super) fn open_named(path: &Path, options: &mut OpenOptions) -> io::Result<File> {
    options.mode(0o600).open(path)
}

pub(super) fn anonymous(dir: &Path) -> io::Result<File> {
    #[cfg(target_os = "linux")]
    match OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_TMPFILE)
        .open(dir)
    {
        Ok(file) => return Ok(file),
        Err(error)
            if matches!(
                error.raw_os_error(),
                Some(libc::EOPNOTSUPP | libc::EISDIR | libc::ENOENT)
            ) => {}
        Err(error) => return Err(error),
    }
    let temporary = Builder::new().tempfile_in(dir)?;
    let super::NamedTempFile { file, path } = temporary;
    drop(path);
    Ok(file)
}

pub(super) fn reopen(file: &File, path: &Path) -> io::Result<File> {
    let reopened = OpenOptions::new().read(true).write(true).open(path)?;
    let old = file.metadata()?;
    let new = reopened.metadata()?;
    if (old.dev(), old.ino()) != (new.dev(), new.ino()) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "original tempfile has been replaced",
        ));
    }
    Ok(reopened)
}

pub(super) fn persist(source: &Path, destination: &Path, overwrite: bool) -> io::Result<()> {
    if overwrite {
        return fs::rename(source, destination);
    }
    static NO_SYSCALL: AtomicBool = AtomicBool::new(false);
    if !NO_SYSCALL.load(Relaxed) {
        match no_replace(source, destination) {
            Ok(()) => return Ok(()),
            Err(error) if error.raw_os_error() == Some(libc::ENOSYS) => {
                NO_SYSCALL.store(true, Relaxed);
            }
            Err(error)
                if error.kind() == io::ErrorKind::Unsupported
                    || error.raw_os_error() == Some(libc::EINVAL) => {}
            Err(error) => return Err(error),
        }
    }
    hard_link(source, destination)?;
    let _ = fs::remove_file(source);
    Ok(())
}
