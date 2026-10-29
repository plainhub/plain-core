//! Unit tests for `src/media_uuid.rs` — moved out-of-line; compiled
//! as the `tests` child module via `#[cfg(test)] #[path]` there.
use super::*;

#[test]
fn filesystem_identity_keys_preserve_existing_xxh64_index_bytes() {
    for (fsuuid, expected) in [
        ("", "media:fid:ef46db3751d8e999:42:-1"),
        ("/", "media:fid:e89cd67289eddaea:42:-1"),
        ("abc-123", "media:fid:d8e7b1339ddd9706:42:-1"),
        ("存储卷", "media:fid:35c46fe2093b7897:42:-1"),
        (
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "media:fid:e8ffbbd2a30a89ca:42:-1",
        ),
    ] {
        assert_eq!(fid_key(fsuuid, 42, -1), expected);
        let directory = tempfile::tempdir().unwrap();
        let db = crate::storage::kv::Db::open(directory.path()).unwrap();
        db.insert(expected.as_bytes(), b"existing-media-id")
            .unwrap();
        assert_eq!(
            find_uuid_by_fid(&db, fsuuid, 42, -1).as_deref(),
            Some("existing-media-id")
        );
        assert_eq!(find_uuid_by_fid(&db, fsuuid, 43, -1), None);
        assert_eq!(find_uuid_by_fid(&db, fsuuid, 42, 0), None);
    }
}

#[test]
fn uuid_from_triplet_deterministic() {
    let a = uuid_from_triplet("abc-123", 42, 1000);
    let b = uuid_from_triplet("abc-123", 42, 1000);
    assert_eq!(a, b);
    assert_eq!(a.len(), 36); // UUID format with dashes
    assert_eq!(a.chars().filter(|c| *c == '-').count(), 4);
}

#[test]
fn uuid_from_triplet_different_inputs() {
    let a = uuid_from_triplet("abc", 1, 100);
    let b = uuid_from_triplet("abc", 2, 100);
    assert_ne!(a, b);
}

#[test]
fn decode_fstab_escapes_works() {
    assert_eq!(decode_fstab_escapes("hello\\040world"), "hello world");
    assert_eq!(decode_fstab_escapes("a\\011b"), "a\tb");
}

#[test]
fn file_identity_returns_inode_and_ctime() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("test.txt");
    std::fs::write(&p, b"hello").unwrap();
    let (ino, ctime) = identity_from_metadata(&std::fs::symlink_metadata(&p).unwrap());
    assert!(ino > 0);
    assert!(ctime > 0);
}

#[test]
fn generate_uuid_from_metadata_matches_shape() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.mp3");
    std::fs::write(&p, b"x").unwrap();
    let meta = std::fs::symlink_metadata(&p).unwrap();
    let path = p.to_str().unwrap();
    let (id, fsuuid, ino, ctime) = generate_uuid_from_metadata(path, &meta);
    assert_eq!(id.len(), 36);
    assert!(!fsuuid.is_empty());
    assert!(ino > 0);
    assert!(ctime > 0);
    assert_eq!(
        id,
        uuid_from_triplet(&fsuuid, ino, ctime),
        "uuid must be the deterministic triplet hash"
    );
}
