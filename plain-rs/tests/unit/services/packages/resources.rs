use super::*;

fn header(kind: u16, size: usize, body: Vec<u8>) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(kind.to_le_bytes());
    bytes.extend((size as u16).to_le_bytes());
    bytes.extend(((8 + body.len()) as u32).to_le_bytes());
    bytes.extend(body);
    bytes
}
fn strings(values: &[&str]) -> Vec<u8> {
    let mut data = Vec::new();
    let mut offsets = Vec::new();
    for value in values {
        offsets.push(data.len() as u32);
        data.extend([value.encode_utf16().count() as u8, value.len() as u8]);
        data.extend(value.as_bytes());
        data.push(0);
    }
    while data.len() % 4 != 0 {
        data.push(0);
    }
    let mut body = Vec::new();
    for value in [values.len() as u32, 0, 256, 28 + 4 * values.len() as u32, 0] {
        body.extend(value.to_le_bytes());
    }
    for offset in offsets {
        body.extend(offset.to_le_bytes());
    }
    body.extend(data);
    header(1, 28, body)
}
fn entry(kind: u8, data: u32, compact: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if compact {
        bytes.extend(0u16.to_le_bytes());
        bytes.extend((((kind as u16) << 8) | 8).to_le_bytes());
        bytes.extend(data.to_le_bytes());
    } else {
        bytes.extend(8u16.to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(8u16.to_le_bytes());
        bytes.extend([0, kind]);
        bytes.extend(data.to_le_bytes());
    }
    bytes
}
fn typed(
    flags: u8,
    language: [u8; 2],
    country: [u8; 2],
    density: u16,
    values: &[(u16, u8, u32, bool)],
) -> Vec<u8> {
    let count = if flags & 1 != 0 {
        values.len()
    } else {
        values
            .iter()
            .map(|value| value.0 as usize + 1)
            .max()
            .unwrap_or(0)
    };
    let width = if flags & 2 != 0 { 2 } else { 4 };
    let start = 36 + count * width;
    let start = (start + 3) & !3;
    let mut offsets = Vec::new();
    let mut payload = Vec::new();
    for (index, kind, data, compact) in values {
        offsets.push((*index, payload.len() as u32));
        payload.extend(entry(*kind, *data, *compact));
    }
    let mut body = Vec::new();
    body.extend([1, flags, 0, 0]);
    body.extend((count as u32).to_le_bytes());
    body.extend((start as u32).to_le_bytes());
    body.extend(16u32.to_le_bytes());
    body.extend([0; 4]);
    body.extend(language);
    body.extend(country);
    body.extend([0; 2]);
    body.extend(density.to_le_bytes());
    if flags & 1 != 0 {
        for (id, offset) in offsets {
            body.extend(id.to_le_bytes());
            body.extend(((offset / 4) as u16).to_le_bytes());
        }
    } else {
        for id in 0..count {
            let offset = offsets
                .iter()
                .find(|(index, _)| *index as usize == id)
                .map(|(_, offset)| *offset);
            if flags & 2 != 0 {
                body.extend(
                    offset
                        .map(|offset| (offset / 4) as u16)
                        .unwrap_or(u16::MAX)
                        .to_le_bytes(),
                );
            } else {
                body.extend(offset.unwrap_or(u32::MAX).to_le_bytes());
            }
        }
    }
    while body.len() + 8 < start {
        body.push(0);
    }
    body.extend(payload);
    header(0x201, 36, body)
}
fn table(chunks: Vec<Vec<u8>>, values: &[&str]) -> Vec<u8> {
    let types = strings(&["string"]);
    let keys = strings(&["app_name"]);
    let mut package = vec![0; 280];
    package[0..4].copy_from_slice(&0x7fu32.to_le_bytes());
    package[260..264].copy_from_slice(&288u32.to_le_bytes());
    package[268..272].copy_from_slice(&(288u32 + types.len() as u32).to_le_bytes());
    package.extend(types);
    package.extend(keys);
    package.extend(chunks.concat());
    let package = header(0x200, 288, package);
    let mut body = 1u32.to_le_bytes().to_vec();
    body.extend(strings(values));
    body.extend(package);
    header(2, 12, body)
}

#[test]
fn english_us_selection_precedes_other_locales_and_density() {
    let bytes = table(
        vec![
            typed(0, [0, 0], [0, 0], 0, &[(0, 3, 0, false)]),
            typed(0, *b"en", *b"US", 160, &[(0, 3, 1, false)]),
            typed(0, *b"fr", *b"FR", 640, &[(0, 3, 2, false)]),
        ],
        &["default", "US text", "French"],
    );
    let resources = Resources::parse(&bytes).unwrap();
    assert_eq!(
        resources.resolve(0x7f010000).unwrap().as_deref(),
        Some("US text")
    );
    assert_eq!(
        resources.name(0x7f010000).unwrap().as_deref(),
        Some("@string/app_name")
    );
}
#[test]
fn dense_sparse_offset16_and_compact_entries_resolve_the_same_id() {
    for flags in [0, 1, 2] {
        for compact in [false, true] {
            let bytes = table(
                vec![typed(flags, [0, 0], [0, 0], 0, &[(7, 3, 0, compact)])],
                &["value"],
            );
            let resources = Resources::parse(&bytes).unwrap();
            assert_eq!(
                resources.resolve(0x7f010007).unwrap().as_deref(),
                Some("value")
            );
            assert_eq!(resources.resolve(0x7f010006).unwrap(), None);
        }
    }
}
#[test]
fn reference_chains_are_bounded_and_cycles_are_errors() {
    let bytes = table(
        vec![typed(
            0,
            [0, 0],
            [0, 0],
            0,
            &[(0, 1, 0x7f010001, false), (1, 3, 0, false)],
        )],
        &["resolved"],
    );
    let resources = Resources::parse(&bytes).unwrap();
    assert_eq!(
        resources.resolve(0x7f010000).unwrap().as_deref(),
        Some("resolved")
    );
    let bytes = table(
        vec![typed(
            0,
            [0, 0],
            [0, 0],
            0,
            &[(0, 1, 0x7f010001, false), (1, 1, 0x7f010000, false)],
        )],
        &["unused"],
    );
    assert!(
        Resources::parse(&bytes)
            .unwrap()
            .resolve(0x7f010000)
            .is_err()
    );
}
#[test]
fn repeated_resource_references_do_not_decode_the_value_again() {
    let bytes = table(
        vec![typed(0, [0, 0], [0, 0], 0, &[(0, 3, 0, false)])],
        &["cached"],
    );
    let resources = Resources::parse(&bytes).unwrap();
    let before = resources.strings.remaining_bytes();
    assert_eq!(
        resources.resolve(0x7f010000).unwrap().as_deref(),
        Some("cached")
    );
    let after = resources.strings.remaining_bytes();
    assert!(after < before);
    for _ in 0..100 {
        assert_eq!(
            resources.resolve(0x7f010000).unwrap().as_deref(),
            Some("cached")
        );
    }
    assert_eq!(resources.strings.remaining_bytes(), after);
}
#[test]
fn malformed_extents_sparse_ids_and_entry_offsets_fail_without_panicking() {
    let bytes = table(
        vec![typed(0, [0, 0], [0, 0], 0, &[(0, 3, 0, false)])],
        &["value"],
    );
    for end in 0..bytes.len() {
        assert!(Resources::parse(&bytes[..end]).is_err());
    }
    assert!(chunk(Bytes::Slice(&bytes), usize::MAX).is_err());
    let bytes = table(
        vec![typed(
            1,
            [0, 0],
            [0, 0],
            0,
            &[(7, 3, 0, false), (1, 3, 0, false)],
        )],
        &["value"],
    );
    assert!(Resources::parse(&bytes).is_err());
}

pub(crate) fn sample_resource_table() -> Vec<u8> {
    table(
        vec![typed(0, *b"en", *b"US", 160, &[(0, 3, 0, false)])],
        &["Localized title"],
    )
}

#[test]
fn invalid_selected_entry_offsets_are_rejected_lazily() {
    let mut bytes = table(
        vec![typed(0, [0, 0], [0, 0], 0, &[(0, 3, 0, false)])],
        &["value"],
    );
    let (at, header) = {
        let resources = Resources::parse(&bytes).unwrap();
        let table = &resources.types[&0x7f01][0];
        let Bytes::Slice(table_bytes) = table.bytes else {
            panic!("Memory fixture required")
        };
        (
            table_bytes.as_ptr() as usize - bytes.as_ptr() as usize,
            table.header,
        )
    };
    bytes[at + header..at + header + 4].copy_from_slice(&(u32::MAX - 1).to_le_bytes());
    assert!(
        Resources::parse(&bytes)
            .unwrap()
            .resolve(0x7f010000)
            .is_err()
    );
}

#[test]
fn a_large_unused_chunk_does_not_consume_the_resource_read_budget() {
    use super::super::input::{ReadAt, ResourceReader};
    use std::{cell::Cell, io::Write};
    struct Counted<'a> {
        input: &'a dyn ReadAt,
        read: Cell<usize>,
    }
    impl ReadAt for Counted<'_> {
        fn read_at(&self, offset: usize, bytes: &mut [u8]) -> Result<()> {
            self.read.set(self.read.get() + bytes.len());
            self.input.read_at(offset, bytes)
        }
    }
    let mut data = sample_resource_table();
    let size = data.len() + 8 + 64 * 1024 * 1024;
    data[4..8].copy_from_slice(&(size as u32).to_le_bytes());
    data.extend(0x7777u16.to_le_bytes());
    data.extend(8u16.to_le_bytes());
    data.extend((64u32 * 1024 * 1024 + 8).to_le_bytes());
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(&data).unwrap();
    file.set_len(size as u64).unwrap();
    let reader = ResourceReader::new(file).unwrap();
    let counted = Counted {
        input: &reader,
        read: Cell::new(0),
    };
    let resources = Resources::from_bytes(Bytes::Reader {
        reader: &counted,
        offset: 0,
        length: size,
    })
    .unwrap();
    assert_eq!(
        resources.resolve(0x7f010000).unwrap().as_deref(),
        Some("Localized title")
    );
    assert!(
        counted.read.get() < 4096,
        "read {} bytes",
        counted.read.get()
    );
}

#[test]
fn the_best_matching_configuration_does_not_read_lower_priority_entries() {
    use super::super::input::ReadAt;
    struct Counted<'a> {
        bytes: &'a [u8],
        reads: RefCell<Vec<std::ops::Range<usize>>>,
    }
    impl ReadAt for Counted<'_> {
        fn read_at(&self, offset: usize, bytes: &mut [u8]) -> Result<()> {
            let range = offset..offset + bytes.len();
            bytes.copy_from_slice(&self.bytes[range.clone()]);
            self.reads.borrow_mut().push(range);
            Ok(())
        }
    }
    let bytes = table(
        vec![
            typed(0, [0, 0], [0, 0], 0, &[(0, 3, 0, false), (1, 3, 0, false)]),
            typed(0, *b"en", *b"US", 160, &[(0, 3, 1, false)]),
            typed(0, *b"en", *b"US", 160, &[(0, 3, 2, false)]),
        ],
        &["default", "first US text", "second US text"],
    );
    let counted = Counted {
        bytes: &bytes,
        reads: RefCell::new(Vec::new()),
    };
    let resources = Resources::from_bytes(Bytes::Reader {
        reader: &counted,
        offset: 0,
        length: bytes.len(),
    })
    .unwrap();
    counted.reads.borrow_mut().clear();
    let value = resources.value(0x7f010000).unwrap().unwrap();
    assert_eq!(
        value.data, 1,
        "Equal configuration scores preserve their original order"
    );
    let Bytes::Reader { offset, length, .. } = value.table.bytes else {
        panic!("Reader fixture required")
    };
    assert!(
        counted
            .reads
            .borrow()
            .iter()
            .all(|range| range.start >= offset && range.end <= offset + length),
        "Lower priority configurations were read"
    );
    assert_eq!(
        resources.resolve(0x7f010001).unwrap().as_deref(),
        Some("default"),
        "A missing best-priority entry falls through to the next configuration"
    );
}
