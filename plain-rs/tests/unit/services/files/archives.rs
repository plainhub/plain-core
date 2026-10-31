use super::*;
use std::io::{Cursor, Read};

#[tokio::test]
async fn streamed_archives_preserve_many_entries_and_repeated_source_bytes() {
    for count in [2, 129] {
        let directory = tempfile::tempdir().unwrap();
        let archives = Archives::new(directory.path().join("staging"));
        let (_stop, stopped) = watch::channel(false);
        let mut entries = Vec::new();
        for index in 0..count {
            let path = directory.path().join(format!("source-{index}.txt"));
            std::fs::write(&path, format!("data-{index}")).unwrap();
            entries.push((path, format!("entry-{index}.txt")));
        }
        entries.push((entries[0].0.clone(), "duplicate-source.txt".into()));
        let mut reader = archives
            .build(entries, archives.acquire().unwrap(), stopped)
            .await
            .unwrap();
        let mut bytes = Vec::new();
        while let Some(chunk) = reader.read().await.unwrap() {
            bytes.extend(chunk);
        }
        assert_eq!(reader.size as usize, bytes.len());
        drop(reader);
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.len(), count + 1);
        for index in 0..count {
            let mut bytes = String::new();
            archive
                .by_name(&format!("entry-{index}.txt"))
                .unwrap()
                .read_to_string(&mut bytes)
                .unwrap();
            assert_eq!(bytes, format!("data-{index}"));
        }
        let mut duplicate = String::new();
        archive
            .by_name("duplicate-source.txt")
            .unwrap()
            .read_to_string(&mut duplicate)
            .unwrap();
        assert_eq!(duplicate, "data-0");
    }
}

#[test]
fn malformed_names_fail_before_sources_are_read_or_staging_is_created() {
    for name in [
        "../outside.txt",
        "/outside.txt",
        "nested/../outside",
        "nested/./file",
        "C:/outside",
        "C:outside",
        "nested\\outside",
        ".",
        "nul\0name",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let staging = directory.path().join("staging");
        let missing = directory.path().join("missing-source");
        let error = build_archive(
            &staging,
            vec![
                (missing.clone(), "valid.txt".into()),
                (missing, name.into()),
            ],
        )
        .err()
        .unwrap();
        assert_eq!(error.to_string(), "Invalid archive entry name", "{name}");
        assert!(!staging.exists());
    }
}

#[tokio::test]
async fn unnamed_media_entries_use_the_source_basename() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("original.txt");
    std::fs::write(&path, b"original bytes").unwrap();
    let archives = Archives::new(directory.path().join("staging"));
    let (_stop, stopped) = watch::channel(false);
    let mut reader = archives
        .build(
            vec![(path, String::new())],
            archives.acquire().unwrap(),
            stopped,
        )
        .await
        .unwrap();
    let mut bytes = Vec::new();
    while let Some(chunk) = reader.read().await.unwrap() {
        bytes.extend(chunk);
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = Vec::new();
    archive
        .by_name("original.txt")
        .unwrap()
        .read_to_end(&mut output)
        .unwrap();
    assert_eq!(output, b"original bytes");
}
