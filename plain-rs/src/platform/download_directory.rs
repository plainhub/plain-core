use std::path::PathBuf;

pub fn resolve() -> Option<PathBuf> {
    #[cfg(windows)]
    return windows::resolve();
    #[cfg(target_os = "macos")]
    return std::env::home_dir().map(|home| home.join("Downloads"));
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let home = std::env::home_dir()?;
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home.join(".config"));
        let bytes = std::fs::read(config.join("user-dirs.dirs")).ok()?;
        xdg(&home, &bytes)
    }
    #[cfg(not(any(unix, windows)))]
    None
}

#[cfg(all(unix, any(test, not(target_os = "macos"))))]
fn xdg(home: &std::path::Path, bytes: &[u8]) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    for line in bytes.split(|byte| *byte == b'\n') {
        let Some(equal) = line.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        if line[..equal].trim_ascii() != b"XDG_DOWNLOAD_DIR" {
            continue;
        }
        let value = line[equal + 1..].trim_ascii();
        let Some(value) = value
            .strip_prefix(b"\"")
            .and_then(|v| v.strip_suffix(b"\""))
        else {
            continue;
        };
        let (base, value) = if let Some(relative) = value.strip_prefix(b"$HOME/") {
            if relative.is_empty() {
                continue;
            }
            (home.to_path_buf(), relative)
        } else if value.starts_with(b"/") {
            (PathBuf::new(), value)
        } else {
            continue;
        };
        let mut decoded = Vec::with_capacity(value.len());
        let mut bytes = value.iter().copied();
        while let Some(byte) = bytes.next() {
            if byte == b'\\' {
                if let Some(byte) = bytes.next() {
                    decoded.push(byte);
                }
            } else {
                decoded.push(byte);
            }
        }
        return Some(base.join(std::ffi::OsString::from_vec(decoded)));
    }
    None
}

#[cfg(windows)]
mod windows;

#[cfg(all(test, unix))]
#[path = "../../tests/unit/platform/download_directory.rs"]
mod tests;
