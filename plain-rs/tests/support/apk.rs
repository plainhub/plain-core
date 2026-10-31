use std::io::{Cursor, Write};

pub fn binary(package: &str, split: Option<&str>, utf8: bool) -> Vec<u8> {
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
pub fn apk(package: &str, split: Option<&str>) -> Vec<u8> {
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
