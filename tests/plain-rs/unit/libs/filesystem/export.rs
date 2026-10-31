use super::*;

#[test]
fn export_overwrites_only_the_selected_file_and_keeps_same_path_bytes() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let destination = root.path().join("export.txt");
    let neighbor = root.path().join("neighbor.txt");
    let bytes = b"shared\0document\n";
    fs::write(&source, bytes).unwrap();
    fs::write(&destination, b"previous").unwrap();
    fs::write(&neighbor, b"keep").unwrap();
    copy_file(&source, &destination).unwrap();
    assert_eq!(fs::read(&destination).unwrap(), bytes);
    copy_file(&source, &source).unwrap();
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(fs::read(&neighbor).unwrap(), b"keep");
}

#[test]
fn export_creates_the_selected_file_without_renaming_on_collision() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let destination = root.path().join("export.txt");
    fs::write(&source, b"document").unwrap();
    copy_file(&source, &destination).unwrap();
    copy_file(&source, &destination).unwrap();
    assert_eq!(fs::read(destination).unwrap(), b"document");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
}

#[test]
fn export_rejects_invalid_sources_and_destinations_without_changing_existing_bytes() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let destination = root.path().join("export.txt");
    fs::write(&source, b"source").unwrap();
    fs::write(&destination, b"keep").unwrap();
    for invalid in [
        root.path().to_path_buf(),
        root.path().join("missing"),
        PathBuf::from("relative"),
    ] {
        assert!(copy_file(&invalid, &destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"keep");
    }
    for invalid in [root.path().to_path_buf(), PathBuf::from("relative")] {
        assert!(copy_file(&source, &invalid).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"source");
    }
}

#[cfg(any(unix, windows))]
#[test]
fn export_to_a_hard_link_preserves_both_names_and_exact_source_bytes() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let alias = root.path().join("alias.txt");
    fs::write(&source, b"do not truncate").unwrap();
    fs::hard_link(&source, &alias).unwrap();
    copy_file(&source, &alias).unwrap();
    assert_eq!(fs::read(source).unwrap(), b"do not truncate");
    assert_eq!(fs::read(alias).unwrap(), b"do not truncate");
}

#[cfg(unix)]
#[test]
fn export_through_source_and_destination_symlinks_keeps_source_bytes() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let alias = root.path().join("alias.txt");
    fs::write(&source, b"document").unwrap();
    std::os::unix::fs::symlink(&source, &alias).unwrap();
    copy_file(&source, &alias).unwrap();
    copy_file(&alias, &source).unwrap();
    assert_eq!(fs::read(&source).unwrap(), b"document");
    assert!(fs::symlink_metadata(&alias).unwrap().is_symlink());
}
