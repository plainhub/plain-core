use super::*;
use std::io::Write;

#[test]
fn archive_entry_bytes_are_exact_and_repeated_reads_do_not_replace_the_cache() {
    let directory = tempfile::tempdir().unwrap();
    let archive = directory.path().join("files.zip");
    let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
    zip.start_file("nested/note.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"actual entry bytes").unwrap();
    zip.finish().unwrap();
    let reference = format!("{}!zip!/nested/note.txt", archive.display());
    let cache = directory.path().join("cache");
    let path = extract(&cache, &reference).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"actual entry bytes");
    #[cfg(unix)]
    let before = {
        use std::os::unix::fs::MetadataExt;
        path.metadata().unwrap().ino()
    };
    assert_eq!(extract(&cache, &reference).unwrap(), path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(path.metadata().unwrap().ino(), before);
    }
    assert_eq!(std::fs::read(&path).unwrap(), b"actual entry bytes");
    assert!(extract(&cache, &format!("{}!zip!/missing.txt", archive.display())).is_err());
    assert_eq!(std::fs::read_dir(cache).unwrap().count(), 1);
}

fn write_entry(archive: &Path, name: &str, bytes: &[u8]) {
    let mut zip = zip::ZipWriter::new(File::create(archive).unwrap());
    zip.start_file(
        name,
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .unwrap();
    zip.write_all(bytes).unwrap();
    zip.finish().unwrap();
}

#[test]
fn changed_archive_replaces_the_preview_identity_and_preserves_old_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let archive = directory.path().join("source.zip");
    let cache = directory.path().join("cache");
    let reference = format!("{}!zip!/note.txt", archive.display());
    write_entry(&archive, "note.txt", b"first");
    let first = extract(&cache, &reference).unwrap();
    write_entry(&archive, "note.txt", b"replacement bytes");
    let second = extract(&cache, &reference).unwrap();
    assert_ne!(first, second);
    assert_eq!(std::fs::read(first).unwrap(), b"first");
    assert_eq!(std::fs::read(second).unwrap(), b"replacement bytes");
}

#[test]
fn leading_slash_entries_and_preview_paths_keep_app_compatibility() {
    let directory = tempfile::tempdir().unwrap();
    let archive = directory.path().join("source.zip");
    let cache = directory.path().join("cache");
    write_entry(&archive, "/nested/note.txt", b"preview");
    let first = extract(
        &cache,
        &format!("{}!zip!/nested/note.txt", archive.display()),
    )
    .unwrap();
    assert_eq!(
        extract(
            &cache,
            &format!("{}!zip!//nested/note.txt/", archive.display())
        )
        .unwrap(),
        first
    );
    assert_eq!(std::fs::read(first).unwrap(), b"preview");
}

#[test]
fn corrupt_entry_never_commits_or_leaves_a_partial_cache_file() {
    let directory = tempfile::tempdir().unwrap();
    let archive = directory.path().join("source.zip");
    let cache = directory.path().join("cache");
    write_entry(&archive, "note.txt", b"unique-content-for-crc-check");
    let mut bytes = std::fs::read(&archive).unwrap();
    let offset = bytes
        .windows(b"unique-content-for-crc-check".len())
        .position(|value| value == b"unique-content-for-crc-check")
        .unwrap();
    bytes[offset] ^= 1;
    std::fs::write(&archive, bytes).unwrap();
    assert!(extract(&cache, &format!("{}!zip!/note.txt", archive.display())).is_err());
    assert_eq!(std::fs::read_dir(cache).unwrap().count(), 0);
}
