use super::*;
use std::{os::unix::ffi::OsStrExt, path::Path};

#[test]
fn xdg_preserves_custom_absolute_and_localized_download_paths() {
    let home = Path::new("/home/example");
    assert_eq!(
        xdg(home, b"XDG_DOWNLOAD_DIR=\"$HOME/Downloads\""),
        Some(home.join("Downloads"))
    );
    assert_eq!(
        xdg(home, "XDG_DOWNLOAD_DIR=\"$HOME/下载\"".as_bytes()),
        Some(home.join("下载"))
    );
    assert_eq!(
        xdg(home, b"\tXDG_DOWNLOAD_DIR = \"/mnt/downloads\" \t"),
        Some(PathBuf::from("/mnt/downloads"))
    );
}

#[test]
fn xdg_rejects_disabled_missing_relative_and_unquoted_values() {
    for bytes in [
        b"".as_slice(),
        b"XDG_DOWNLOAD_DIR=\"$HOME/\"",
        b"XDG_DOWNLOAD_DIR=\"$HOME\"",
        b"XDG_DOWNLOAD_DIR=Downloads",
        b"XDG_DOWNLOAD_DIR=\"Downloads\"",
        b"#XDG_DOWNLOAD_DIR=\"/tmp/downloads\"",
    ] {
        assert_eq!(xdg(Path::new("/home/example"), bytes), None);
    }
}

#[test]
fn xdg_decodes_literal_shell_escapes_without_expanding_or_executing_them() {
    let value = xdg(
        Path::new("/home/example"),
        br#"XDG_DOWNLOAD_DIR="$HOME/a\"b\$c\`d\\e""#,
    )
    .unwrap();
    assert_eq!(value, Path::new("/home/example/a\"b$c`d\\e"));
    assert_eq!(
        xdg(
            Path::new("/home/example"),
            b"XDG_DOWNLOAD_DIR=\"/mnt/\xff\"\n"
        )
        .unwrap()
        .as_os_str()
        .as_bytes(),
        b"/mnt/\xff"
    );
}

#[test]
fn xdg_uses_the_first_valid_download_entry() {
    let bytes = b"XDG_MUSIC_DIR=\"/music\"\nXDG_DOWNLOAD_DIR=invalid\nXDG_DOWNLOAD_DIR=\"/first\"\nXDG_DOWNLOAD_DIR=\"/second\"\n";
    assert_eq!(
        xdg(Path::new("/home/example"), bytes),
        Some(PathBuf::from("/first"))
    );
}

#[cfg(target_os = "macos")]
#[test]
fn native_download_directory_matches_home_downloads() {
    assert_eq!(
        resolve(),
        std::env::home_dir().map(|home| home.join("Downloads"))
    );
}
