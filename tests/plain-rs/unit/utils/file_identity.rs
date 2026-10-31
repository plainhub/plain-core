use super::*;

#[test]
fn physical_identity_survives_rename_and_content_changes() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original.txt");
    let moved = root.path().join("moved.txt");
    std::fs::write(&original, b"before").unwrap();
    let before = key(&original).unwrap();
    std::fs::rename(&original, &moved).unwrap();
    assert_eq!(before, key(&moved).unwrap());
    std::fs::write(&moved, b"after with a different length").unwrap();
    assert_eq!(before, key(&moved).unwrap());
}

#[test]
fn hard_links_are_the_same_file_but_independent_copies_are_not() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original.txt");
    let linked = root.path().join("linked.txt");
    let copied = root.path().join("copied.txt");
    std::fs::write(&original, b"same bytes").unwrap();
    std::fs::hard_link(&original, &linked).unwrap();
    std::fs::copy(&original, &copied).unwrap();
    assert!(same(&original, &linked).unwrap());
    assert!(!same(&original, &copied).unwrap());
    assert_eq!(key(&original).unwrap(), key(&linked).unwrap());
}

#[test]
fn missing_sources_are_errors_without_creating_any_files() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing.txt");
    assert_eq!(key(&missing).unwrap_err().kind(), io::ErrorKind::NotFound);
    assert!(!missing.exists());
}
