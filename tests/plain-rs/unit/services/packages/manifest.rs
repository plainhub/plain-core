use super::*;
use std::io::{Cursor, Write};

pub(crate) fn binary(package: &str, split: Option<&str>, utf8: bool) -> Vec<u8> {
    let mut values = vec!["manifest", "package", package];
    if let Some(split) = split {
        values.extend(["split", split]);
    }
    let mut data = Vec::new();
    let mut offsets = Vec::new();
    for value in &values {
        offsets.push(data.len() as u32);
        if utf8 {
            let size = value.encode_utf16().count();
            for size in [size, value.len()] {
                if size >= 128 {
                    data.push(0x80 | ((size >> 8) as u8));
                }
                data.push(size as u8);
            }
            data.extend(value.as_bytes());
            data.push(0);
        } else {
            data.extend((value.encode_utf16().count() as u16).to_le_bytes());
            for value in value.encode_utf16() {
                data.extend(value.to_le_bytes());
            }
            data.extend(0u16.to_le_bytes());
        }
    }
    while data.len() % 4 != 0 {
        data.push(0);
    }
    let pool_size = 28 + 4 * values.len() + data.len();
    let mut pool = Vec::new();
    pool.extend(1u16.to_le_bytes());
    pool.extend(28u16.to_le_bytes());
    pool.extend((pool_size as u32).to_le_bytes());
    for value in [
        values.len() as u32,
        0,
        if utf8 { 256 } else { 0 },
        (28 + 4 * values.len()) as u32,
        0,
    ] {
        pool.extend(value.to_le_bytes());
    }
    for offset in offsets {
        pool.extend(offset.to_le_bytes());
    }
    pool.extend(data);
    let count = if split.is_some() { 2u16 } else { 1 };
    let mut node = Vec::new();
    node.extend(0x102u16.to_le_bytes());
    node.extend(16u16.to_le_bytes());
    node.extend((36u32 + 20 * count as u32).to_le_bytes());
    node.extend(1u32.to_le_bytes());
    node.extend(u32::MAX.to_le_bytes());
    node.extend(u32::MAX.to_le_bytes());
    node.extend(0u32.to_le_bytes());
    for value in [20u16, 20, count, 0, 0, 0] {
        node.extend(value.to_le_bytes());
    }
    for (name, value) in [(1u32, 2u32)]
        .into_iter()
        .chain(split.map(|_| (3u32, 4u32)))
    {
        for value in [u32::MAX, name, value] {
            node.extend(value.to_le_bytes());
        }
        node.extend(8u16.to_le_bytes());
        node.extend([0, 3]);
        node.extend(value.to_le_bytes());
    }
    node.extend(0x103u16.to_le_bytes());
    node.extend(16u16.to_le_bytes());
    node.extend(24u32.to_le_bytes());
    for value in [1u32, u32::MAX, u32::MAX, 0] {
        node.extend(value.to_le_bytes());
    }
    let mut bytes = Vec::new();
    bytes.extend(3u16.to_le_bytes());
    bytes.extend(8u16.to_le_bytes());
    bytes.extend((8u32 + pool.len() as u32 + node.len() as u32).to_le_bytes());
    bytes.extend(pool);
    bytes.extend(node);
    bytes
}
pub(crate) fn apk(package: &str, split: Option<&str>) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "AndroidManifest.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    writer.write_all(&binary(package, split, true)).unwrap();
    writer.finish().unwrap().into_inner()
}

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
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "AndroidManifest.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    writer
        .write_all(&binary("com.example.package", None, true))
        .unwrap();
    writer
        .start_file(
            "resources.arsc",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
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
