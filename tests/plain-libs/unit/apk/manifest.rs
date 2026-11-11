use super::*;
use std::io::{Cursor, Write};

use crate::test_apk::{apk, binary};

#[test]
fn manifest_identity_supports_utf8_utf16_base_and_split_without_resources() {
    for utf8 in [false, true] {
        let base = root(&binary("com.example.package", None, utf8)).unwrap();
        assert_eq!(base.package_name, "com.example.package");
        assert!(base.split.is_none());
        let split = root(&binary(
            "com.example.package",
            Some("config.arm64_v8a"),
            utf8,
        ))
        .unwrap();
        assert_eq!(split.split.as_deref(), Some("config.arm64_v8a"));
    }
    assert_eq!(
        read(Cursor::new(apk("com.example.package", None)))
            .unwrap()
            .package_name,
        "com.example.package"
    );
}

#[test]
fn manifest_strings_support_extended_lengths_and_multibyte_values() {
    for utf8 in [true, false] {
        let split = format!("{}雪", "x".repeat(130));
        assert_eq!(
            root(&binary("com.example.package", Some(&split), utf8))
                .unwrap()
                .split,
            Some(split)
        );
    }
}

#[test]
fn malformed_extents_offsets_indexes_and_identity_are_rejected() {
    let bytes = binary("com.example.package", None, true);
    for end in 0..bytes.len() {
        assert!(root(&bytes[..end]).is_err());
    }
    let mut bad = bytes.clone();
    bad[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(root(&bad).is_err());
    let mut bad = bytes.clone();
    let start = 8 + u32_at(&bad, 12).unwrap() as usize;
    bad[start + 40..start + 44].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(root(&bad).is_err());
    assert!(root(&binary("", None, true)).is_err());
    assert!(root(&binary("bad/package", None, true)).is_err());
}

#[test]
fn sparse_pool_does_not_decode_unrequested_string_offsets() {
    let mut bytes = binary("com.example.package", None, true);
    let header = 8;
    let pool = Pool::new(&bytes[header..], 28).unwrap();
    assert_eq!(pool.get(2).unwrap(), "com.example.package");
    let expected = MANIFEST_LIMIT as usize - "com.example.package".len();
    assert_eq!(pool.remaining.get(), expected);
    bytes[header + 28..header + 32].copy_from_slice(&u32::MAX.to_le_bytes());
    let pool = Pool::new(&bytes[header..], 28).unwrap();
    assert_eq!(pool.get(2).unwrap(), "com.example.package");
    assert!(pool.get(0).is_err());
}

#[test]
fn full_manifest_without_resource_references_skips_unrelated_resource_payload() {
    use std::{cell::Cell, rc::Rc};
    struct Counted {
        input: Cursor<Vec<u8>>,
        read: Rc<Cell<usize>>,
    }
    impl Read for Counted {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            let count = self.input.read(bytes)?;
            self.read.set(self.read.get() + count);
            Ok(count)
        }
    }
    impl Seek for Counted {
        fn seek(&mut self, offset: std::io::SeekFrom) -> std::io::Result<u64> {
            self.input.seek(offset)
        }
    }
    let mut writer = crate::zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "AndroidManifest.xml",
            crate::zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    writer
        .write_all(&binary("com.example.package", None, true))
        .unwrap();
    writer
        .start_file(
            "resources.arsc",
            crate::zip::write::SimpleFileOptions::default()
                .compression_method(crate::zip::CompressionMethod::Stored),
        )
        .unwrap();
    writer.write_all(&vec![0u8; 1024 * 1024]).unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    let read = Rc::new(Cell::new(0));
    let output = xml(Counted {
        input: Cursor::new(bytes),
        read: read.clone(),
    })
    .unwrap();
    assert!(output.contains("com.example.package"));
    assert!(read.get() < 128 * 1024, "read {} bytes", read.get());
}
