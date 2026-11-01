use super::*;
use std::io::{Cursor, Read};

#[tokio::test]
async fn streamed_archives_preserve_many_entries_and_repeated_source_bytes() {
    for count in [2, 129] {
        let directory = tempfile::tempdir().unwrap();
        let archives = Archives::new(crate::infrastructure::cache_paths::CachePaths::new(
            &directory.path().join("cache"),
        ));
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
    let archives = Archives::new(crate::infrastructure::cache_paths::CachePaths::new(
        &directory.path().join("cache"),
    ));
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

#[tokio::test]
async fn archive_preview_is_readable_without_exposing_private_archive_staging() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("data")).unwrap();
    let state = core.runtime_state();
    let source = directory.path().join("owned.zip");
    let mut archive = zip::ZipWriter::new(std::fs::File::create(&source).unwrap());
    archive
        .start_file("owned.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    archive.write_all(b"exact preview bytes").unwrap();
    archive.finish().unwrap();
    let preview = state
        .archives
        .preview(
            &state.file_platform,
            format!("{}!zip!/owned.txt", source.display()),
        )
        .await
        .unwrap();
    assert!(
        crate::features::services::files::links::authorize_path(&state, preview.to_str().unwrap())
            .is_ok()
    );
    assert_eq!(std::fs::read(&preview).unwrap(), b"exact preview bytes");
    let private = state.cache.root().join("archives/private-staging");
    std::fs::create_dir_all(private.parent().unwrap()).unwrap();
    std::fs::write(&private, b"private bytes").unwrap();
    assert!(
        crate::features::services::files::links::authorize_path(&state, private.to_str().unwrap())
            .is_err()
    );
    assert!(!state.directory.join("archives").exists());
    core.stop_core().await;
}
