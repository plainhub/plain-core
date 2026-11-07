// Adapted from sysinfo 0.32.1; see SOURCE.json and LICENSE.

use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};
pub(crate) fn get_all_utf8_data<P: AsRef<Path>>(file_path: P, size: usize) -> io::Result<String> {
    let mut file = File::open(file_path.as_ref())?;
    let mut value = String::with_capacity(size);
    file.read_to_string(&mut value)?;
    Ok(value)
}
pub(crate) fn to_u64(bytes: &[u8]) -> u64 {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}
pub(crate) fn to_cpath(path: &std::path::Path) -> Vec<u8> {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    let path_os: &OsStr = path.as_ref();
    let mut cpath = path_os.as_bytes().to_vec();
    cpath.push(0);
    cpath
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/system_metrics/unix/linux/utils.rs"]
mod tests;
