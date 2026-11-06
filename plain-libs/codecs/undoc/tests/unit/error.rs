use super::*;

#[test]
fn test_error_display() {
    let err = Error::UnknownFormat;
    assert_eq!(err.to_string(), "Unknown file format");

    let err = Error::UnsupportedFormat("legacy .doc".to_string());
    assert_eq!(err.to_string(), "Unsupported format: legacy .doc");
}

#[test]
fn test_error_from_io() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    let err: Error = io_err.into();
    assert!(matches!(err, Error::Io(_)));
}

#[test]
fn test_error_kind_mapping() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    assert_eq!(Error::from(io_err).kind(), ErrorKind::Io);
    assert_eq!(Error::UnknownFormat.kind(), ErrorKind::UnknownFormat);
    assert_eq!(
        Error::UnsupportedFormat("legacy .doc".into()).kind(),
        ErrorKind::UnsupportedFormat
    );
    assert_eq!(
        Error::ZipArchive("bad central directory".into()).kind(),
        ErrorKind::ZipArchive
    );
    assert_eq!(
        Error::InvalidData("bad".into()).kind(),
        ErrorKind::InvalidData
    );
    assert_eq!(
        Error::MissingComponent("[Content_Types].xml".into()).kind(),
        ErrorKind::MissingComponent
    );
    assert_eq!(
        Error::Encoding("bad utf-8".into()).kind(),
        ErrorKind::Encoding
    );
    assert_eq!(
        Error::StyleNotFound("Heading1".into()).kind(),
        ErrorKind::StyleNotFound
    );
    assert_eq!(
        Error::ResourceNotFound("rId1".into()).kind(),
        ErrorKind::ResourceNotFound
    );
    assert_eq!(Error::Encrypted.kind(), ErrorKind::Encrypted);
    assert_eq!(Error::Render("bad table".into()).kind(), ErrorKind::Render);
    assert_eq!(
        Error::SectionOutOfRange { index: 3, count: 2 }.kind(),
        ErrorKind::SectionOutOfRange
    );
}

/// Both XML variants are the same failure reason, so they must share one number —
/// otherwise the ABI carries two values for one reason.
#[test]
fn test_xml_parse_variants_share_one_kind() {
    assert_eq!(
        Error::XmlParse("unexpected eof".into()).kind(),
        ErrorKind::XmlParse
    );
    assert_eq!(
        Error::xml_parse_with_context("unexpected eof", "word/document.xml").kind(),
        ErrorKind::XmlParse
    );
}

/// The ZIP layer knows more than "something went wrong with the archive", and this
/// is the only place that knowledge still exists. Constructing the `ZipError`
/// values directly tests every branch without having to fabricate an archive that
/// provokes each one — including the password cases, which are otherwise awkward to
/// produce.
#[test]
fn test_zip_error_reasons_are_not_collapsed() {
    use zip::result::ZipError;

    assert_eq!(
        Error::from(ZipError::InvalidArchive("no central directory".into())).kind(),
        ErrorKind::ZipArchive,
        "a damaged container stays a damaged container"
    );
    assert_eq!(
        Error::from(ZipError::UnsupportedArchive("zip64 with disks")).kind(),
        ErrorKind::UnsupportedFormat,
        "unsupported is not the same problem as damaged"
    );
    assert_eq!(
        Error::from(ZipError::InvalidPassword).kind(),
        ErrorKind::Encrypted
    );
    assert_eq!(
        Error::from(ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED)).kind(),
        ErrorKind::Encrypted,
        "the ZIP spec reports a password-protected entry as unsupported"
    );
    assert_eq!(
        Error::from(ZipError::FileNotFound).kind(),
        ErrorKind::MissingComponent
    );
    assert_eq!(
        Error::from(ZipError::Io(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "eof"
        )))
        .kind(),
        ErrorKind::Io
    );
}

/// XML that fails to arrive, to decode, and to parse are three different problems.
#[test]
fn test_xml_error_reasons_are_not_collapsed() {
    let io_backed: Error = quick_xml::Error::Io(std::sync::Arc::new(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "eof mid-element",
    )))
    .into();
    assert_eq!(io_backed.kind(), ErrorKind::Io);

    let malformed: Error =
        quick_xml::Error::Syntax(quick_xml::errors::SyntaxError::UnclosedTag).into();
    assert_eq!(malformed.kind(), ErrorKind::XmlParse);
}

/// Serializing output is rendering. Reporting it as an XML parse failure sent a
/// caller looking at the input document for a problem that is not there.
#[test]
fn test_json_serialization_failure_is_classified_as_render() {
    let err = Error::Render("JSON serialization: recursion limit".to_string());
    assert_eq!(err.kind(), ErrorKind::Render);
}
