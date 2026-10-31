use super::*;

fn node(kind: u16, data: Vec<u8>) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(kind.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend((16u32 + data.len() as u32).to_le_bytes());
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(u32::MAX.to_le_bytes());
    bytes.extend(data);
    bytes
}
fn pair(kind: u16, first: u32, second: u32) -> Vec<u8> {
    node(kind, [first.to_le_bytes(), second.to_le_bytes()].concat())
}
fn start(namespace: u32, tag: u32, attributes: &[(u32, u32, u32, u8, u32)]) -> Vec<u8> {
    let mut data = [namespace.to_le_bytes(), tag.to_le_bytes()].concat();
    for value in [20u16, 20, attributes.len() as u16, 0, 0, 0] {
        data.extend(value.to_le_bytes());
    }
    for (ns, name, raw, kind, value) in attributes {
        for value in [*ns, *name, *raw] {
            data.extend(value.to_le_bytes());
        }
        data.extend(8u16.to_le_bytes());
        data.extend([0, *kind]);
        data.extend(value.to_le_bytes());
    }
    node(0x102, data)
}
fn document(strings: &[&str], nodes: Vec<Vec<u8>>) -> Vec<u8> {
    let mut content = Vec::new();
    let mut offsets = Vec::new();
    for text in strings {
        offsets.push(content.len() as u32);
        for count in [text.encode_utf16().count(), text.len()] {
            if count >= 128 {
                content.push(0x80 | (count >> 8) as u8);
            }
            content.push(count as u8);
        }
        content.extend(text.as_bytes());
        content.push(0);
    }
    while content.len() % 4 != 0 {
        content.push(0);
    }
    let mut pool = Vec::new();
    pool.extend(1u16.to_le_bytes());
    pool.extend(28u16.to_le_bytes());
    pool.extend((28u32 + 4 * strings.len() as u32 + content.len() as u32).to_le_bytes());
    for value in [
        strings.len() as u32,
        0,
        256,
        28 + 4 * strings.len() as u32,
        0,
    ] {
        pool.extend(value.to_le_bytes());
    }
    for offset in offsets {
        pool.extend(offset.to_le_bytes());
    }
    pool.extend(content);
    let mut body = pool;
    body.extend(nodes.concat());
    let mut bytes = Vec::new();
    bytes.extend(3u16.to_le_bytes());
    bytes.extend(8u16.to_le_bytes());
    bytes.extend((8u32 + body.len() as u32).to_le_bytes());
    bytes.extend(body);
    bytes
}
#[test]
fn metadata_and_full_xml_share_the_same_utf_string_reader() {
    for utf8 in [false, true] {
        let output = render(&super::super::manifest::tests::binary(
            "com.example.package",
            None,
            utf8,
        ))
        .unwrap();
        assert_eq!(
            output,
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<manifest package=\"com.example.package\">\n</manifest>\n"
        );
    }
}
#[test]
fn namespaces_escaping_and_child_structure_survive_rendering() {
    let bytes = document(
        &[
            "manifest",
            "android",
            "http://schemas.android.com/apk/res/android",
            "application",
            "label",
            "A&B<\"雪",
        ],
        vec![
            pair(0x100, 1, 2),
            start(u32::MAX, 0, &[]),
            start(u32::MAX, 3, &[(2, 4, 5, 3, 5)]),
            pair(0x103, u32::MAX, 3),
            pair(0x103, u32::MAX, 0),
            pair(0x101, 1, 2),
        ],
    );
    let output = render(&bytes).unwrap();
    assert!(output.contains("xmlns:android=\"http://schemas.android.com/apk/res/android\""));
    assert!(output.contains("android:label=\"A&amp;B&lt;&quot;雪\""));
    assert!(output.contains("</application>\n</manifest>"));
}
#[test]
fn typed_values_preserve_ids_booleans_signed_numbers_colors_dimensions_and_fractions() {
    let pool_bytes = document(&["manifest"], Vec::new());
    let pool = Pool::new(&pool_bytes[8..], 28).unwrap();
    for (kind, data, expected) in [
        (1, 0x7f010001, "@0x7f010001"),
        (2, 0x01010001, "?0x01010001"),
        (0x12, 1, "true"),
        (0x10, u32::MAX, "-1"),
        (0x11, 15, "0xf"),
        (0x1e, 0xff112233, "#ff112233"),
        (5, (24 << 8) | 1, "24dp"),
        (6, (1 << 8) | 1, "100%p"),
        (0, 1, "@empty"),
    ] {
        assert_eq!(typed(&pool, kind, data).unwrap(), expected);
    }
    assert!(typed(&pool, 4, f32::INFINITY.to_bits()).is_err());
    assert!(typed(&pool, 0xff, 0).is_err());
}
#[test]
fn default_namespace_attributes_receive_an_explicit_prefix() {
    let bytes = document(
        &["manifest", "urn:example", "value", "text"],
        vec![
            pair(0x100, u32::MAX, 1),
            start(1, 0, &[(1, 2, 3, 3, 3)]),
            pair(0x103, 1, 0),
            pair(0x101, u32::MAX, 1),
        ],
    );
    let output = render(&bytes).unwrap();
    assert!(output.contains("xmlns=\"urn:example\""));
    assert!(output.contains("xmlns:ns0=\"urn:example\""));
    assert!(output.contains("ns0:value=\"text\""));
}
#[test]
fn duplicate_expanded_attributes_and_unbalanced_documents_are_rejected() {
    let strings = ["manifest", "value", "text", "value"];
    let invalid = vec![
        vec![
            start(
                u32::MAX,
                0,
                &[(u32::MAX, 1, 2, 3, 2), (u32::MAX, 3, 2, 3, 2)],
            ),
            pair(0x103, u32::MAX, 0),
        ],
        vec![start(u32::MAX, 0, &[])],
        vec![start(u32::MAX, 0, &[]), pair(0x103, u32::MAX, 1)],
        vec![
            start(u32::MAX, 0, &[]),
            pair(0x103, u32::MAX, 0),
            start(u32::MAX, 0, &[]),
            pair(0x103, u32::MAX, 0),
        ],
        vec![start(2, 0, &[])],
    ];
    for nodes in invalid {
        assert!(render(&document(&strings, nodes)).is_err());
    }
}
#[test]
fn namespace_rebinding_generates_an_alias_for_a_shadowed_uri() {
    let bytes = document(
        &[
            "manifest",
            "p",
            "urn:outer",
            "urn:inner",
            "application",
            "value",
            "text",
        ],
        vec![
            pair(0x100, 1, 2),
            start(u32::MAX, 0, &[]),
            pair(0x100, 1, 3),
            start(u32::MAX, 4, &[(2, 5, 6, 3, 6)]),
            pair(0x103, u32::MAX, 4),
            pair(0x101, 1, 3),
            pair(0x103, u32::MAX, 0),
            pair(0x101, 1, 2),
        ],
    );
    let output = render(&bytes).unwrap();
    assert!(output.contains("xmlns:ns0=\"urn:outer\""));
    assert!(output.contains("ns0:value=\"text\""));
}

#[test]
fn resource_and_framework_style_values_are_rendered_with_their_names() {
    let table = super::super::resources::tests::sample_resource_table();
    let resources = super::super::resources::Resources::parse(&table).unwrap();
    let bytes = document(
        &[
            "manifest",
            "android",
            "http://schemas.android.com/apk/res/android",
            "application",
            "label",
            "theme",
        ],
        vec![
            pair(0x100, 1, 2),
            start(u32::MAX, 0, &[]),
            start(
                u32::MAX,
                3,
                &[
                    (2, 4, u32::MAX, 1, 0x7f010000),
                    (2, 5, u32::MAX, 1, 0x01030000),
                ],
            ),
            pair(0x103, u32::MAX, 3),
            pair(0x103, u32::MAX, 0),
            pair(0x101, 1, 2),
        ],
    );
    assert!(needs_resources(&bytes).unwrap());
    let output = render_with(&bytes, Some(&resources)).unwrap();
    assert!(output.contains("android:label=\"Localized title\""));
    assert!(output.contains("android:theme=\"@android:style/Animation\""));
    for compression in [
        zip::CompressionMethod::Stored,
        zip::CompressionMethod::Deflated,
    ] {
        use std::io::{Cursor, Write};
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default().compression_method(compression);
        writer.start_file("AndroidManifest.xml", options).unwrap();
        writer.write_all(&bytes).unwrap();
        writer.start_file("resources.arsc", options).unwrap();
        writer.write_all(&table).unwrap();
        let actual = super::super::manifest::xml(writer.finish().unwrap()).unwrap();
        assert_eq!(actual, output);
    }
}

#[test]
fn resource_map_recovers_empty_framework_attribute_names_without_arsc() {
    let mut bytes = document(
        &[
            "manifest",
            "android",
            "http://schemas.android.com/apk/res/android",
            "application",
            "",
            "Title",
        ],
        vec![
            pair(0x100, 1, 2),
            start(u32::MAX, 0, &[]),
            start(u32::MAX, 3, &[(2, 4, 5, 3, 5)]),
            pair(0x103, u32::MAX, 3),
            pair(0x103, u32::MAX, 0),
            pair(0x101, 1, 2),
        ],
    );
    let at = 8 + u32_at(&bytes, 12).unwrap() as usize;
    let mut map = Vec::new();
    map.extend(0x180u16.to_le_bytes());
    map.extend(8u16.to_le_bytes());
    map.extend(32u32.to_le_bytes());
    for id in [0, 0, 0, 0, 0x01010001u32, 0] {
        map.extend(id.to_le_bytes());
    }
    bytes.splice(at..at, map);
    let length = bytes.len() as u32;
    bytes[4..8].copy_from_slice(&length.to_le_bytes());
    assert!(!needs_resources(&bytes).unwrap());
    assert!(render(&bytes).unwrap().contains("android:label=\"Title\""));
}

#[test]
fn text_nodes_use_the_raw_string_when_the_typed_value_is_empty() {
    let text = node(0x104, [2u32.to_le_bytes().as_slice(), &[0; 8]].concat());
    let bytes = document(
        &["manifest", "unused", "A&B<雪"],
        vec![start(u32::MAX, 0, &[]), text, pair(0x103, u32::MAX, 0)],
    );
    assert!(render(&bytes).unwrap().contains("A&amp;B&lt;雪\n"));
}

#[test]
fn text_nodes_validate_the_representation_they_read() {
    let render_text = |data| {
        render(&document(
            &["manifest", "text"],
            vec![
                start(u32::MAX, 0, &[]),
                node(0x104, data),
                pair(0x103, u32::MAX, 0),
            ],
        ))
    };
    let typed = [8u16.to_le_bytes().as_slice(), &[0, 3], &1u32.to_le_bytes()].concat();
    let valid = [u32::MAX.to_le_bytes().as_slice(), typed.as_slice()].concat();
    assert!(render_text(valid.clone()).unwrap().contains("text\n"));
    let mut missing = valid.clone();
    missing[4..6].copy_from_slice(&0u16.to_le_bytes());
    assert!(render_text(missing).is_err());
    let mut reserved = valid;
    reserved[6] = 1;
    assert!(render_text(reserved).is_err());
    assert!(render_text([2u32.to_le_bytes().as_slice(), &[0; 8]].concat()).is_err());
    assert!(render_text(1u32.to_le_bytes().to_vec()).is_err());
}

#[test]
fn stored_resource_tables_read_referenced_values_without_buffering_unused_chunks() {
    use std::io::{Seek, SeekFrom, Write};
    let manifest = document(
        &["manifest", "label"],
        vec![
            start(u32::MAX, 0, &[(u32::MAX, 1, u32::MAX, 1, 0x7f010000)]),
            pair(0x103, u32::MAX, 0),
        ],
    );
    let mut table = super::super::resources::tests::sample_resource_table();
    let padding = 64 * 1024 * 1024;
    let total = table.len() + 8 + padding;
    table[4..8].copy_from_slice(&(total as u32).to_le_bytes());
    let mut writer = zip::ZipWriter::new(tempfile::tempfile().unwrap());
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("AndroidManifest.xml", options).unwrap();
    writer.write_all(&manifest).unwrap();
    writer.start_file("resources.arsc", options).unwrap();
    writer.write_all(&table).unwrap();
    writer.write_all(&0x7777u16.to_le_bytes()).unwrap();
    writer.write_all(&8u16.to_le_bytes()).unwrap();
    writer
        .write_all(&((padding + 8) as u32).to_le_bytes())
        .unwrap();
    for _ in 0..padding / 65536 {
        writer.write_all(&[0; 65536]).unwrap();
    }
    let mut file = writer.finish().unwrap();
    let output = super::super::manifest::xml(&mut file).unwrap();
    assert!(output.contains("label=\"Localized title\""));
    let corrupt_at = {
        let mut archive = zip::ZipArchive::new(&mut file).unwrap();
        archive
            .by_name("resources.arsc")
            .unwrap()
            .data_start()
            .unwrap()
            + table.len() as u64
            + 9
    };
    file.seek(SeekFrom::Start(corrupt_at)).unwrap();
    file.write_all(&[1]).unwrap();
    assert!(
        super::super::manifest::xml(file).is_err(),
        "Skipped resource payload must still pass ZIP CRC verification"
    );
}
