use super::*;
use std::io::{Cursor, Read, Write};
#[test]
fn archive_entries_keep_windows_and_parent_path_safety_on_every_host() {
    let cases = [
        ("safe/file.txt", Some("safe/file.txt")),
        ("a/../file.txt", Some("file.txt")),
        ("../secret", None),
        ("a/../../secret", None),
        (r"b\C:\secret", None),
        (r"\\?\C:\a/../secret", None),
        ("C:/safe.txt", Some("safe.txt")),
        ("c:d:file.txt", Some("file.txt")),
        (r"\\server\share\safe.txt", Some("safe.txt")),
    ];
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, _) in cases {
        writer
            .start_file(
                name,
                write::SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(b"payload").unwrap();
    }
    let mut archive = ZipArchive::new(writer.finish().unwrap()).unwrap();
    for (index, (name, expected)) in cases.into_iter().enumerate() {
        let file = archive.by_index(index).unwrap();
        assert_eq!(file.name().unwrap(), name);
        assert_eq!(
            file.enclosed_name(),
            expected.map(std::path::PathBuf::from),
            "{name}"
        );
    }
}
#[test]
fn stored_and_deflated_entries_keep_bytes_crc_and_metadata() {
    for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = write::SimpleFileOptions::default()
            .compression_method(method)
            .unix_permissions(0o640);
        writer.start_file("data.txt", options).unwrap();
        writer
            .write_all(b"repeated payload repeated payload")
            .unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let mut archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let mut file = archive.by_name("data.txt").unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        assert_eq!(data, b"repeated payload repeated payload");
        assert_eq!(file.unix_mode().unwrap() & 0o777, 0o640);
        if method == CompressionMethod::Stored {
            let mut bad = bytes.clone();
            let offset = bad
                .windows(16)
                .position(|w| w == b"repeated payload")
                .unwrap();
            bad[offset] ^= 1;
            let mut bad = ZipArchive::new(Cursor::new(bad)).unwrap();
            assert!(
                bad.by_index(0)
                    .unwrap()
                    .read_to_end(&mut Vec::new())
                    .is_err()
            );
        }
        for size in [0, 1, 4, bytes.len() / 2] {
            assert!(ZipArchive::new(Cursor::new(&bytes[..size])).is_err());
        }
    }
}
#[cfg(windows)]
#[test]
fn windows_reserved_names_streams_and_trailing_aliases_stay_rejected() {
    for name in [
        "CON.txt",
        "a/NUL",
        "file:Zone.Identifier",
        "bad.exe.",
        "bad ",
        "COM¹.txt",
    ] {
        assert!(path::enclosed_name(name).is_none(), "{name}");
    }
}
