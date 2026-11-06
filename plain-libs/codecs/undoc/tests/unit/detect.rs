#[test]
fn all_variants_are_listed() {
    // The match is what does the work. Adding a variant to FormatType without adding it
    // to ALL stops this from compiling -- which is the only thing keeping a consumer's
    // copy of the extension list from going quietly stale behind a new format.
    for format in FormatType::ALL {
        match format {
            FormatType::Docx
            | FormatType::Xlsx
            | FormatType::Pptx
            | FormatType::Doc
            | FormatType::Xls
            | FormatType::Ppt => {}
        }
    }

    // The list is only useful to a consumer if each entry maps to a distinct extension.
    let mut extensions: Vec<&str> = FormatType::ALL.iter().map(|f| f.extension()).collect();
    extensions.sort_unstable();
    extensions.dedup();
    assert_eq!(extensions.len(), FormatType::ALL.len());
    assert_eq!(extensions, ["doc", "docx", "ppt", "pptx", "xls", "xlsx"]);

    // Both fields are pinned here rather than only at the binding that serialises them:
    // this test runs on every host, whereas the wasm crate's does not always build
    // locally. What a consumer reads is (extension, name), so that pair is the contract.
    let pairs: Vec<(&str, &str)> = FormatType::ALL
        .iter()
        .map(|f| (f.extension(), f.name()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("docx", "Word Document"),
            ("xlsx", "Excel Workbook"),
            ("pptx", "PowerPoint Presentation"),
            ("doc", "Word 97-2003 Document"),
            ("xls", "Excel 97-2003 Workbook"),
            ("ppt", "PowerPoint 97-2003 Presentation"),
        ]
    );
}

use super::*;

#[test]
fn test_format_type_display() {
    assert_eq!(FormatType::Docx.to_string(), "Word Document");
    assert_eq!(FormatType::Xlsx.to_string(), "Excel Workbook");
    assert_eq!(FormatType::Pptx.to_string(), "PowerPoint Presentation");
}

#[test]
fn test_format_type_extension() {
    assert_eq!(FormatType::Docx.extension(), "docx");
    assert_eq!(FormatType::Xlsx.extension(), "xlsx");
    assert_eq!(FormatType::Pptx.extension(), "pptx");
}

#[test]
fn test_is_zip_file() {
    assert!(is_zip_file(&[0x50, 0x4B, 0x03, 0x04, 0x00]));
    assert!(!is_zip_file(&[0x00, 0x00, 0x00, 0x00]));
    assert!(!is_zip_file(&[0x50, 0x4B])); // Too short
}

#[test]
fn test_detect_invalid_data() {
    let result = detect_format_from_bytes(&[0x00, 0x00, 0x00, 0x00]);
    assert!(matches!(result, Err(Error::UnknownFormat)));
}

/// A legacy binary Office file — or an ECMA-376 encrypted one — is a container we
/// recognise and cannot open. Reporting it as unrecognised sends the caller looking
/// for a file-type problem they do not have, and letting it reach the ZIP layer
/// reports it as damaged, which is worse: it invites a pointless repair attempt.
#[test]
fn test_ole_cfb_container_is_unsupported_not_unknown_or_damaged() {
    let mut data = CFB_MAGIC.to_vec();
    data.extend_from_slice(&[0u8; 64]);

    let err = detect_format_from_bytes(&data).unwrap_err();

    assert_eq!(
        err.kind(),
        crate::ErrorKind::UnsupportedFormat,
        "got: {err}"
    );
}

/// A CFB whose header is intact but whose directory sector is not readable.
///
/// Built by filling the first directory sector of a valid container with `0xFF`. The
/// header still passes every check that gets the file this far -- magic, version,
/// sector size -- so the failure is exactly the one the branch documents: a container
/// that is provably an Office file and provably unopenable. A truncated header does
/// not exercise that; it fails before the directory is ever read.
fn cfb_with_unreadable_directory() -> Vec<u8> {
    const SECTOR_SHIFT_OFFSET: usize = 0x1E;
    const FIRST_DIR_SECTOR_OFFSET: usize = 0x30;

    let mut data = cfb_with_streams(&["/WordDocument"]);
    let le_u16 = |at: usize| u16::from_le_bytes(data[at..at + 2].try_into().unwrap());
    let le_u32 = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());

    // Sector size is declared, not assumed: this crate's containers are version 4
    // (4096-byte sectors), while a file written by Office 97-2003 is version 3 (512).
    let sector_size = 1usize << le_u16(SECTOR_SHIFT_OFFSET);
    let first_dir_sector = le_u32(FIRST_DIR_SECTOR_OFFSET) as usize;
    // The header occupies sector 0, so sector N begins one sector later.
    let start = sector_size * (first_dir_sector + 1);
    assert!(
        start + sector_size <= data.len(),
        "the directory sector should lie inside the file"
    );

    data[start..start + sector_size].fill(0xFF);
    data
}

/// Build a CFB container holding the named root streams, and nothing else.
fn cfb_with_streams(names: &[&str]) -> Vec<u8> {
    let mut container =
        cfb::CompoundFile::create(std::io::Cursor::new(Vec::new())).expect("create CFB container");
    for name in names {
        container.create_stream(name).expect("create stream");
    }
    container.flush().expect("flush CFB container");
    container.into_inner().into_inner()
}

/// The distinction this branch exists for. Both files carry the CFB header, and the
/// two answers send the caller in opposite directions — a password versus a
/// converter — so reporting the disjunction leaves them to guess.
#[test]
fn test_ecma376_encrypted_package_is_encrypted_not_unsupported() {
    let data = cfb_with_streams(&["/EncryptedPackage", "/EncryptionInfo"]);

    // Same header as a legacy binary file, so the directory is what decides.
    assert_eq!(data[..CFB_MAGIC.len()], CFB_MAGIC);

    let err = detect_format_from_bytes(&data).unwrap_err();

    assert_eq!(err.kind(), crate::ErrorKind::Encrypted, "got: {err}");
    assert!(matches!(err, Error::Encrypted));
}

#[test]
fn test_word_binary_document_is_detected_as_doc() {
    let format = detect_format_from_bytes(&cfb_with_streams(&["/WordDocument"])).unwrap();
    assert_eq!(format, FormatType::Doc);
}

#[test]
fn test_excel_binary_workbook_is_detected_as_xls() {
    let format = detect_format_from_bytes(&cfb_with_streams(&["/Workbook"])).unwrap();
    assert_eq!(format, FormatType::Xls);
}

#[test]
fn test_powerpoint_binary_presentation_is_detected_as_ppt() {
    let format = detect_format_from_bytes(&cfb_with_streams(&["/PowerPoint Document"])).unwrap();
    assert_eq!(format, FormatType::Ppt);
}

/// Excel 5.0/95 keeps its workbook in a `Book` stream rather than `Workbook`.
#[test]
fn test_excel_95_workbook_is_detected_as_xls() {
    let format = detect_format_from_bytes(&cfb_with_streams(&["/Book"])).unwrap();
    assert_eq!(format, FormatType::Xls);
}

/// Excel 2.x–4.0 saved the record stream with no container: the BOF record is the
/// signature. Anything else that starts with those two bytes is not taken for one.
#[test]
fn test_bare_biff_streams_are_detected_as_xls() {
    for head in [
        &[0x09, 0x00, 0x04, 0x00, 0x02, 0x00, 0x10, 0x00][..],
        &[0x09, 0x02, 0x06, 0x00, 0x00, 0x00, 0x10, 0x00, 0, 0],
        &[0x09, 0x04, 0x06, 0x00, 0x00, 0x00, 0x00, 0x01, 0, 0],
    ] {
        assert_eq!(detect_format_from_bytes(head).unwrap(), FormatType::Xls);
    }
    for not_biff in [
        &[0x09, 0x04, 0x07, 0x00, 0x00, 0x00, 0x10, 0x00, 0, 0][..], // wrong length
        &[0x09, 0x04, 0x06, 0x00, 0x00, 0x00, 0x99, 0x00, 0, 0],     // no such substream
        &[0x09, 0x04, 0x06],                                         // too short
    ] {
        let err = detect_format_from_bytes(not_biff).unwrap_err();
        assert_eq!(err.kind(), crate::ErrorKind::UnknownFormat, "got: {err}");
    }
}

/// The branch that exists for a container whose header proves it is an Office file
/// and whose directory proves nothing else. It must not claim which kind -- naming one
/// would send the caller after a password or a converter on no evidence -- and it must
/// not be indistinguishable from the "recognised nothing" answer, which is reached only
/// when the directory *was* read.
#[test]
fn test_cfb_with_unreadable_directory_names_neither_kind() {
    let err = detect_format_from_bytes(&cfb_with_unreadable_directory()).unwrap_err();

    assert_eq!(
        err.kind(),
        crate::ErrorKind::UnsupportedFormat,
        "got: {err}"
    );
    let message = err.to_string();
    assert!(
        message.contains("directory could not be read"),
        "should say the directory is what failed: {message}"
    );
    // The same bytes name a Word stream when the directory is intact, so a message
    // that still names it would be reporting a directory it never read.
    assert!(
        !message.contains("Word 97-2003"),
        "must not name a format it could not read: {message}"
    );
}

/// A CFB that is neither encrypted OOXML nor a format we can name stays unsupported.
/// Naming is best-effort; the header is what was proved.
#[test]
fn test_unrecognised_cfb_is_still_unsupported() {
    let err = detect_format_from_bytes(&cfb_with_streams(&["/SomethingElse"])).unwrap_err();

    assert_eq!(
        err.kind(),
        crate::ErrorKind::UnsupportedFormat,
        "got: {err}"
    );
}

/// Truncated input must not be mistaken for a CFB header on a prefix match.
#[test]
fn test_truncated_cfb_prefix_is_unknown_format() {
    let err = detect_format_from_bytes(&CFB_MAGIC[..4]).unwrap_err();

    assert!(matches!(err, Error::UnknownFormat));
}

/// The ZIP path must keep working through the new magic-byte gate, including the
/// damaged-archive case that still has to read as damaged.
#[test]
fn test_zip_magic_still_reaches_the_archive_layer() {
    let mut data = ZIP_MAGIC.to_vec();
    data.extend_from_slice(b"not a real central directory");

    let err = detect_format_from_bytes(&data).unwrap_err();

    assert_eq!(err.kind(), crate::ErrorKind::ZipArchive, "got: {err}");
}

/// A package whose only content is its main part, declared under `content_type`.
fn package(main_part: &str, content_type: &str) -> Vec<u8> {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Override PartName="/{main_part}" ContentType="{content_type}"/>
</Types>"#
    )
    .unwrap();
    zip.start_file(main_part, options).unwrap();
    zip.write_all(b"<root/>").unwrap();
    zip.finish().unwrap().into_inner()
}

/// Each format is told apart by the content type of its main part -- the only thing
/// available for bytes that arrive without a file name.
#[test]
fn test_detects_each_format_from_its_content_type() {
    for (main_part, content_type, expected) in [
        ("word/document.xml", DOCX_CONTENT_TYPE, FormatType::Docx),
        ("xl/workbook.xml", XLSX_CONTENT_TYPE, FormatType::Xlsx),
        ("ppt/presentation.xml", PPTX_CONTENT_TYPE, FormatType::Pptx),
    ] {
        let data = package(main_part, content_type);
        assert_eq!(
            detect_format_from_bytes(&data).unwrap(),
            expected,
            "{main_part}"
        );
    }
}

/// The path-based entry point reads the same bytes from disk.
#[test]
fn test_detects_format_from_a_file_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deck.pptx");
    std::fs::write(&path, package("ppt/presentation.xml", PPTX_CONTENT_TYPE)).unwrap();

    assert_eq!(detect_format_from_path(&path).unwrap(), FormatType::Pptx);
}

#[test]
fn test_format_type_default_is_docx() {
    assert_eq!(FormatType::default(), FormatType::Docx);
}

#[test]
fn test_format_type_serde_roundtrip() {
    let json = serde_json::to_string(&FormatType::Xlsx).unwrap();
    assert_eq!(json, "\"xlsx\"");
    let back: FormatType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, FormatType::Xlsx);
}
