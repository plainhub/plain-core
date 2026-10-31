use super::*;

#[test]
fn cache_key_preserves_unicode_and_invalidates_each_source_fact() {
    let key = pdf_preview_key("/文件/report.docx", 1000, 5000);
    assert_eq!(
        key,
        "14178f252eae71e51aca57dbb2ef24e4d6b34ac8c0e4c995c903656b6731d7bc0deb6016b54a1367c35452be30afe980900e02153f4041ad9890e3d0204f9d06"
    );
    assert_eq!(key, pdf_preview_key("/文件/report.docx", 1000, 5000));
    for changed in [
        pdf_preview_key("/文件/other.docx", 1000, 5000),
        pdf_preview_key("/文件/report.docx", 1001, 5000),
        pdf_preview_key("/文件/report.docx", 1000, 5001),
    ] {
        assert_ne!(key, changed);
    }
}

#[tokio::test]
async fn cached_preview_reuses_its_bytes_without_conversion() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("report.docx");
    std::fs::write(&source, b"source").unwrap();
    let source = source.to_str().unwrap();
    let path = pdf_preview_cache_dir(directory.path())
        .join(format!("{}.pdf", pdf_preview_key(source, 1000, 6)));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"%PDF-1.4 cached preview").unwrap();
    let returned = get_or_create_pdf_preview(directory.path(), source, 1000, 6)
        .await
        .unwrap();
    assert_eq!(returned, path);
    assert_eq!(std::fs::read(returned).unwrap(), b"%PDF-1.4 cached preview");
}
