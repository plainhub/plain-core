use super::*;

#[test]
fn test_resolve_path() {
    assert_eq!(
        OoxmlContainer::resolve_path("word/document.xml", "../media/image1.png"),
        "media/image1.png"
    );
    assert_eq!(
        OoxmlContainer::resolve_path("word/document.xml", "styles.xml"),
        "word/styles.xml"
    );
    assert_eq!(
        OoxmlContainer::resolve_path("xl/worksheets/sheet1.xml", "../sharedStrings.xml"),
        "xl/sharedStrings.xml"
    );
    assert_eq!(
        OoxmlContainer::resolve_path("ppt/slides/slide1.xml", "/ppt/media/image1.png"),
        "ppt/media/image1.png"
    );
}

#[test]
fn test_relationships_collection() {
    let mut rels = Relationships::new();
    rels.add(Relationship {
        id: "rId1".to_string(),
        rel_type: "http://test/type1".to_string(),
        target: "target1.xml".to_string(),
        external: false,
    });
    rels.add(Relationship {
        id: "rId2".to_string(),
        rel_type: "http://test/type1".to_string(),
        target: "target2.xml".to_string(),
        external: false,
    });

    assert!(rels.get("rId1").is_some());
    assert!(rels.get("rId3").is_none());
    assert_eq!(rels.get_by_type("http://test/type1").len(), 2);
}

#[test]
fn test_relationship_projection_helpers_preserve_targets_and_types() {
    let mut rels = Relationships::new();
    rels.add(Relationship {
        id: "rId1".to_string(),
        rel_type: "urn:test:type".to_string(),
        target: "../media/image1.png".to_string(),
        external: false,
    });

    let target_only = rels.clone().into_targets_by_id();
    assert_eq!(
        target_only.get("rId1"),
        Some(&"../media/image1.png".to_string())
    );

    let typed = rels.into_type_targets_by_id();
    assert_eq!(
        typed.get("rId1"),
        Some(&(
            "urn:test:type".to_string(),
            "../media/image1.png".to_string()
        ))
    );
}

/// The listing and the package relationships are what a caller walks to find the main
/// part, so both are pinned to exact values rather than "not empty".
#[test]
fn test_package_listing_and_relationships() {
    let container = create_container_with_files(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
        ),
        ("word/document.xml", "<w:document/>"),
        ("word/styles.xml", "<w:styles/>"),
    ]);

    let mut files = container.list_files();
    files.sort();
    assert_eq!(
        files,
        [
            "[Content_Types].xml",
            "_rels/.rels",
            "word/document.xml",
            "word/styles.xml"
        ]
    );

    let mut word = container.list_files_with_prefix("word/");
    word.sort();
    assert_eq!(word, ["word/document.xml", "word/styles.xml"]);

    assert!(container.exists("word/document.xml"));
    assert!(!container.exists("word/numbering.xml"));

    let rels = container.read_package_relationships().unwrap();
    assert_eq!(rels.by_id.len(), 1);
    assert_eq!(rels.by_id["rId1"].target, "word/document.xml");
}

/// A part of 20 MiB of zeros deflates to about 20 KiB: past a hundredfold, it is read as a
/// damaged archive (a decompression bomb) instead of being allocated. A part of ordinary
/// text is read whole.
#[test]
fn a_part_that_expands_past_a_hundredfold_is_a_damaged_archive() {
    use std::io::{Cursor, Write};
    let mut zip = plain_libs::zip::ZipWriter::new(Cursor::new(Vec::new()));
    let deflated = plain_libs::zip::write::SimpleFileOptions::default()
        .compression_method(plain_libs::zip::CompressionMethod::Deflated);
    zip.start_file("word/document.xml", deflated).unwrap();
    zip.write_all(&vec![b' '; 1 << 20]).unwrap();
    zip.start_file("word/styles.xml", deflated).unwrap();
    zip.write_all(b"<w:styles/>").unwrap();
    let container = OoxmlContainer::from_bytes(zip.finish().unwrap().into_inner()).unwrap();

    match container.read_xml("word/document.xml") {
        Err(Error::ZipArchive(message)) => {
            assert!(message.contains("decompression bomb"), "{message}")
        }
        other => panic!("expected a damaged archive, got {other:?}"),
    }
    assert!(matches!(
        container.read_binary("word/document.xml"),
        Err(Error::ZipArchive(_))
    ));
    assert_eq!(
        container.read_xml("word/styles.xml").unwrap(),
        "<w:styles/>"
    );
}

fn create_container_with_files(files: &[(&str, &str)]) -> OoxmlContainer {
    use std::io::{Cursor, Write};

    let buf = Cursor::new(Vec::new());
    let mut zip = plain_libs::zip::ZipWriter::new(buf);
    let options =
        plain_libs::zip::write::SimpleFileOptions::default().compression_method(plain_libs::zip::CompressionMethod::Stored);

    for (path, contents) in files {
        zip.start_file(*path, options).unwrap();
        zip.write_all(contents.as_bytes()).unwrap();
    }

    OoxmlContainer::from_bytes(zip.finish().unwrap().into_inner()).unwrap()
}

fn create_container_with_binary_files(files: &[(&str, &[u8])]) -> OoxmlContainer {
    use std::io::{Cursor, Write};

    let buf = Cursor::new(Vec::new());
    let mut zip = plain_libs::zip::ZipWriter::new(buf);
    let options =
        plain_libs::zip::write::SimpleFileOptions::default().compression_method(plain_libs::zip::CompressionMethod::Stored);

    for (path, contents) in files {
        zip.start_file(*path, options).unwrap();
        zip.write_all(contents).unwrap();
    }

    OoxmlContainer::from_bytes(zip.finish().unwrap().into_inner()).unwrap()
}

/// Build a ZIP whose entry is flagged as password-protected.
///
/// Bit 0 of the general-purpose bit flag marks an entry encrypted, and it lives in
/// both the local header and the central directory. Flipping it is enough for the
/// ZIP layer to refuse the entry as needing a password — which is the condition we
/// need to reproduce, without pulling in a crypto feature just to write one.
fn create_password_protected_zip_bytes(path: &str) -> Vec<u8> {
    use std::io::{Cursor, Write};

    let buf = Cursor::new(Vec::new());
    let mut zip = plain_libs::zip::ZipWriter::new(buf);
    let options =
        plain_libs::zip::write::SimpleFileOptions::default().compression_method(plain_libs::zip::CompressionMethod::Stored);
    zip.start_file(path, options).unwrap();
    zip.write_all(b"<Types/>").unwrap();
    let mut bytes = zip.finish().unwrap().into_inner();

    // Local file header: signature at 0, flag at offset 6.
    bytes[6] |= 1;
    // Central directory header: flag at offset 8 from its signature.
    let cd = bytes
        .windows(4)
        .position(|w| w == [0x50, 0x4B, 0x01, 0x02])
        .expect("central directory header");
    bytes[cd + 8] |= 1;

    bytes
}

/// A container that needs a password must not be reported as one that is merely
/// missing the part we asked for — the two call for completely different responses
/// from whoever is handling the failure.
#[test]
fn test_password_protected_entry_is_reported_as_encrypted() {
    let bytes = create_password_protected_zip_bytes("word/document.xml");
    let container = OoxmlContainer::from_bytes(bytes).unwrap();

    let err = container.read_xml("word/document.xml").unwrap_err();

    assert_eq!(err.kind(), crate::ErrorKind::Encrypted, "got: {err}");
}

/// The other half of the same contract: a genuinely absent part stays a missing
/// component, so `read_xml_optional` keeps treating it as "not there".
#[test]
fn test_absent_entry_is_still_a_missing_component() {
    let container = create_container_with_files(&[("word/document.xml", "<w:document/>")]);

    let err = container.read_xml("word/styles.xml").unwrap_err();
    assert_eq!(err.kind(), crate::ErrorKind::MissingComponent);
    assert!(matches!(
        container.read_xml_optional("word/styles.xml"),
        Ok(None)
    ));
}

/// And the promise `read_xml_optional` makes in its own doc comment: a part that
/// exists but cannot be read is not silently downgraded to "absent".
#[test]
fn test_optional_part_does_not_swallow_an_unreadable_entry() {
    let bytes = create_password_protected_zip_bytes("word/styles.xml");
    let container = OoxmlContainer::from_bytes(bytes).unwrap();

    let result = container.read_xml_optional("word/styles.xml");

    assert!(
        matches!(&result, Err(e) if e.kind() == crate::ErrorKind::Encrypted),
        "expected the failure to surface, got: {result:?}"
    );
}

#[test]
fn test_parse_relationships_preserves_target_mode_and_start_forms() {
    let rels = parse_relationships_xml(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="urn:test:inline" Target="item1.xml"></Relationship>
  <Relationship Id="rId2" Type="urn:test:external" Target="https://example.com" TargetMode="External"/>
</Relationships>"#,
            "test.rels",
        )
        .unwrap();

    assert_eq!(rels.by_id.len(), 2);
    assert_eq!(rels.by_id["rId1"].target, "item1.xml");
    assert!(!rels.by_id["rId1"].external);
    assert_eq!(rels.by_id["rId2"].target, "https://example.com");
    assert!(rels.by_id["rId2"].external);
}

#[test]
fn test_optional_relationships_allow_missing_but_not_malformed_content() {
    let missing = create_container_with_files(&[]);
    let rels = missing
        .read_optional_relationships_for_part("ppt/slides/slide1.xml")
        .unwrap();
    assert!(rels.by_id.is_empty());

    let malformed =
        create_container_with_files(&[("ppt/slides/_rels/slide1.xml.rels", "<Relationships")]);
    let err = malformed
        .read_optional_relationships_for_part("ppt/slides/slide1.xml")
        .unwrap_err();

    match err {
        Error::XmlParseWithContext { location, .. } => {
            assert_eq!(location, "ppt/slides/_rels/slide1.xml.rels")
        }
        other => panic!("expected malformed optional rels error, got {other:?}"),
    }
}

#[test]
fn test_required_relationships_error_when_missing() {
    let container = create_container_with_files(&[]);
    let err = container
        .read_required_relationships_for_part("word/document.xml")
        .unwrap_err();

    match err {
        Error::MissingComponent(path) => assert_eq!(path, "word/_rels/document.xml.rels"),
        other => panic!("expected missing required rels error, got {other:?}"),
    }
}

#[test]
fn test_utf16_decoding_function() {
    // Test UTF-16 LE with BOM
    let utf16_le = b"\xFF\xFE<\0?\0x\0m\0l\0>\0";
    let result = decode_xml_bytes(utf16_le).expect("Should decode UTF-16 LE");
    assert_eq!(result, "<?xml>");

    // Test UTF-16 BE with BOM
    let utf16_be = b"\xFE\xFF\0<\0?\0x\0m\0l\0>";
    let result = decode_xml_bytes(utf16_be).expect("Should decode UTF-16 BE");
    assert_eq!(result, "<?xml>");

    // Test UTF-8 BOM
    let utf8_bom = b"\xEF\xBB\xBF<?xml>";
    let result = decode_xml_bytes(utf8_bom).expect("Should decode UTF-8 with BOM");
    assert_eq!(result, "<?xml>");

    // Test UTF-8 without BOM
    let utf8_plain = b"<?xml>";
    let result = decode_xml_bytes(utf8_plain).expect("Should decode UTF-8 without BOM");
    assert_eq!(result, "<?xml>");
}

#[test]
fn test_decode_xml_bytes_rejects_invalid_non_utf8_input() {
    let cp1252_xml = b"<?xml version=\"1.0\"?><root>Caf\xe9 & \x93quotes\x94</root>";
    let err = decode_xml_bytes(cp1252_xml).unwrap_err();

    assert!(matches!(err, Error::Encoding(_)));
}

/// BOM-less UTF-16 of ASCII text is a *valid* UTF-8 byte sequence — every second byte
/// is NUL, which UTF-8 accepts as U+0000. Decoding it as UTF-8 therefore succeeds and
/// returns element names no reader can match, so the part parses to nothing without
/// an error. Detection has to come before the UTF-8 attempt, not after it fails.
#[test]
fn test_decode_xml_bytes_detects_bom_less_utf16_of_pure_ascii() {
    let ascii = "<?xml version=\"1.0\"?><root>plain</root>";

    let le: Vec<u8> = ascii.encode_utf16().flat_map(u16::to_le_bytes).collect();
    assert_eq!(
        decode_xml_bytes(&le).expect("BOM-less UTF-16 LE must decode"),
        ascii
    );

    let be: Vec<u8> = ascii.encode_utf16().flat_map(u16::to_be_bytes).collect();
    assert_eq!(
        decode_xml_bytes(&be).expect("BOM-less UTF-16 BE must decode"),
        ascii
    );
}

/// An encoding failure is an encoding failure at every entry point. Reporting some of
/// them as I/O sends consumers a discriminant that says the disk was at fault.
#[test]
fn test_encoding_failures_are_classified_as_encoding_everywhere() {
    // A lone high surrogate: well-formed UTF-16 code units, not a valid scalar.
    let mut lone_surrogate = vec![0xFF, 0xFE];
    lone_surrogate.extend_from_slice(&0xD800u16.to_le_bytes());
    lone_surrogate.extend_from_slice(&0x0041u16.to_le_bytes());
    assert_eq!(
        decode_xml_bytes(&lone_surrogate).unwrap_err().kind(),
        crate::error::ErrorKind::Encoding
    );

    let mut lone_surrogate_be = vec![0xFE, 0xFF];
    lone_surrogate_be.extend_from_slice(&0xD800u16.to_be_bytes());
    lone_surrogate_be.extend_from_slice(&0x0041u16.to_be_bytes());
    assert_eq!(
        decode_xml_bytes(&lone_surrogate_be).unwrap_err().kind(),
        crate::error::ErrorKind::Encoding
    );

    // UTF-8 BOM followed by bytes that are not UTF-8.
    let bad_after_bom = b"\xEF\xBB\xBF<root>Caf\xe9</root>";
    assert_eq!(
        decode_xml_bytes(bad_after_bom).unwrap_err().kind(),
        crate::error::ErrorKind::Encoding
    );
}

/// After decoding, the string is UTF-8 whatever the declaration claimed — including
/// spellings a fixed list of literals would miss.
#[test]
fn test_decoded_part_declares_the_encoding_it_actually_has() {
    for declared in ["UTF-16", "utf-16", "UTF-16LE", "utf-16be", "ISO-8859-1"] {
        let xml = format!("<?xml version=\"1.0\" encoding=\"{declared}\"?><root/>");
        let bytes: Vec<u8> = std::iter::once(0xFEFFu16)
            .chain(xml.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();

        let decoded = decode_xml_bytes(&bytes).expect("must decode");
        assert_eq!(
            decoded, "<?xml version=\"1.0\" encoding=\"UTF-8\"?><root/>",
            "declared {declared} was left in place"
        );
    }
}

/// A declaration with no encoding attribute, and one with none at all, must come back
/// untouched rather than rewritten or dropped.
#[test]
fn test_declaration_without_an_encoding_is_left_alone() {
    for xml in ["<?xml version=\"1.0\"?><root/>", "<root/>"] {
        let bytes: Vec<u8> = std::iter::once(0xFEFFu16)
            .chain(xml.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(decode_xml_bytes(&bytes).expect("must decode"), xml);
    }
}

#[test]
fn test_parse_core_metadata_preserves_whitespace_and_raw_malformed_text() {
    let container = create_container_with_files(&[(
        "docProps/core.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
    xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>  Spaced &bogus; Title  </dc:title>
</cp:coreProperties>"#,
    )]);

    let meta = container.parse_core_metadata().unwrap();
    assert_eq!(meta.title.as_deref(), Some("  Spaced &bogus; Title  "));
}

#[test]
fn test_parse_app_metadata_preserves_whitespace_and_raw_malformed_text() {
    let container = create_container_with_files(&[(
        "docProps/app.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>  App &bogus; Name  </Application>
</Properties>"#,
    )]);

    let meta = container.parse_core_metadata().unwrap();
    assert_eq!(meta.application.as_deref(), Some("  App &bogus; Name  "));
}

#[test]
fn test_parse_core_metadata_propagates_invalid_xml_bytes() {
    let container = create_container_with_binary_files(&[(
            "docProps/core.xml",
            b"<?xml version=\"1.0\"?><cp:coreProperties xmlns:cp=\"urn:test\"><cp:title>Caf\xe9</cp:title></cp:coreProperties>",
        )]);

    let err = container.parse_core_metadata().unwrap_err();
    assert!(matches!(err, Error::Encoding(_)));
}

/// Helper: create a minimal OOXML ZIP archive with given entries.
fn create_test_zip(entries: &[(&str, &str)]) -> Vec<u8> {
    use std::io::Write;
    let buf = Vec::new();
    let cursor = Cursor::new(buf);
    let mut zip = plain_libs::zip::ZipWriter::new(cursor);
    let options =
        plain_libs::zip::write::SimpleFileOptions::default().compression_method(plain_libs::zip::CompressionMethod::Stored);
    for (name, content) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    let cursor = zip.finish().unwrap();
    cursor.into_inner()
}

#[test]
fn test_parse_core_metadata_last_modified_by() {
    let core_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/"
                   xmlns:dcterms="http://purl.org/dc/terms/">
  <dc:title>Test Title</dc:title>
  <dc:creator>Author Name</dc:creator>
  <cp:lastModifiedBy>Editor Name</cp:lastModifiedBy>
  <dcterms:created>2024-01-01T00:00:00Z</dcterms:created>
</cp:coreProperties>"#;

    let data = create_test_zip(&[
        ("[Content_Types].xml", "<Types/>"),
        ("docProps/core.xml", core_xml),
    ]);

    let container = OoxmlContainer::from_bytes(data).unwrap();
    let meta = container.parse_core_metadata().unwrap();

    assert_eq!(meta.title.as_deref(), Some("Test Title"));
    assert_eq!(meta.author.as_deref(), Some("Author Name"));
    assert_eq!(meta.last_modified_by.as_deref(), Some("Editor Name"));
    assert_eq!(meta.created.as_deref(), Some("2024-01-01T00:00:00Z"));
}

#[test]
fn test_parse_app_metadata_basic() {
    let app_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>Microsoft Office Word</Application>
  <Pages>5</Pages>
  <Words>1234</Words>
</Properties>"#;

    let data = create_test_zip(&[
        ("[Content_Types].xml", "<Types/>"),
        ("docProps/app.xml", app_xml),
    ]);

    let container = OoxmlContainer::from_bytes(data).unwrap();
    let meta = container.parse_core_metadata().unwrap();

    assert_eq!(meta.application.as_deref(), Some("Microsoft Office Word"));
    assert_eq!(meta.page_count, Some(5));
    assert_eq!(meta.word_count, Some(1234));
}

#[test]
fn test_parse_app_metadata_slides() {
    let app_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>Microsoft Office PowerPoint</Application>
  <Slides>10</Slides>
</Properties>"#;

    let data = create_test_zip(&[
        ("[Content_Types].xml", "<Types/>"),
        ("docProps/app.xml", app_xml),
    ]);

    let container = OoxmlContainer::from_bytes(data).unwrap();
    let meta = container.parse_core_metadata().unwrap();

    assert_eq!(
        meta.application.as_deref(),
        Some("Microsoft Office PowerPoint")
    );
    assert_eq!(meta.page_count, Some(10));
    assert_eq!(meta.word_count, None);
}

#[test]
fn test_parse_app_metadata_pages_does_not_override_slides() {
    // When both Pages and Slides are present, the first one encountered wins
    // (Pages comes before Slides in the XML), which is correct since
    // DOCX has Pages, PPTX has Slides — they don't coexist in practice.
    let app_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>Test</Application>
  <Pages>3</Pages>
  <Slides>10</Slides>
</Properties>"#;

    let data = create_test_zip(&[
        ("[Content_Types].xml", "<Types/>"),
        ("docProps/app.xml", app_xml),
    ]);

    let container = OoxmlContainer::from_bytes(data).unwrap();
    let meta = container.parse_core_metadata().unwrap();

    // Pages is parsed first, so page_count = 3; Slides won't override it
    assert_eq!(meta.page_count, Some(3));
}

#[test]
fn test_parse_app_metadata_missing_file() {
    // No app.xml — should not fail, just leave fields as None
    let data = create_test_zip(&[("[Content_Types].xml", "<Types/>")]);

    let container = OoxmlContainer::from_bytes(data).unwrap();
    let meta = container.parse_core_metadata().unwrap();

    assert_eq!(meta.application, None);
    assert_eq!(meta.page_count, None);
    assert_eq!(meta.word_count, None);
}

#[test]
fn test_parse_combined_core_and_app_metadata() {
    let core_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>My Document</dc:title>
  <dc:creator>Jane Doe</dc:creator>
  <cp:lastModifiedBy>John Smith</cp:lastModifiedBy>
</cp:coreProperties>"#;

    let app_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>LibreOffice</Application>
  <Pages>12</Pages>
  <Words>5000</Words>
</Properties>"#;

    let data = create_test_zip(&[
        ("[Content_Types].xml", "<Types/>"),
        ("docProps/core.xml", core_xml),
        ("docProps/app.xml", app_xml),
    ]);

    let container = OoxmlContainer::from_bytes(data).unwrap();
    let meta = container.parse_core_metadata().unwrap();

    assert_eq!(meta.title.as_deref(), Some("My Document"));
    assert_eq!(meta.author.as_deref(), Some("Jane Doe"));
    assert_eq!(meta.last_modified_by.as_deref(), Some("John Smith"));
    assert_eq!(meta.application.as_deref(), Some("LibreOffice"));
    assert_eq!(meta.page_count, Some(12));
    assert_eq!(meta.word_count, Some(5000));
}
