use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub fn existing_file(path: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Absolute file path required",
        ));
    }
    let path = fs::canonicalize(path)?;
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Regular file required",
        ));
    }
    Ok(path)
}

pub fn copy_file(source: &Path, destination: &Path) -> io::Result<()> {
    let source = existing_file(source)?;
    if !destination.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Absolute file path required",
        ));
    }
    match fs::metadata(destination) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Regular file required",
                ));
            }
            if super::rename::same_identity(&source, destination)? {
                return Ok(());
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::copy(source, destination)?;
    Ok(())
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/libs/filesystem/export.rs"]
mod tests;
