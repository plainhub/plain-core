use super::*;

#[test]
fn unused_names_stay_unchanged_including_existing_numeric_suffixes() {
    let directory = tempfile::tempdir().unwrap();
    for name in ["a.txt", "a (7).txt", "README", ".gitignore", "报告.txt"] {
        let target = directory.path().join(name);
        assert_eq!(unique_sibling(&target).unwrap(), target);
    }
}

#[test]
fn collisions_use_flat_parenthesized_counters_and_keep_every_existing_file() {
    let directory = tempfile::tempdir().unwrap();
    for name in ["a.txt", "a (1).txt"] {
        std::fs::write(directory.path().join(name), name).unwrap();
    }
    for name in ["a.txt", "a (1).txt"] {
        assert_eq!(
            unique_sibling(&directory.path().join(name)).unwrap(),
            directory.path().join("a (2).txt")
        );
    }
    assert_eq!(
        std::fs::read(directory.path().join("a.txt")).unwrap(),
        b"a.txt"
    );
    assert_eq!(
        std::fs::read(directory.path().join("a (1).txt")).unwrap(),
        b"a (1).txt"
    );
}

#[test]
fn suffixes_preserve_dotfiles_extensionless_unicode_and_the_last_extension() {
    let directory = tempfile::tempdir().unwrap();
    for (name, expected) in [
        ("README", "README (1)"),
        (".gitignore", ".gitignore (1)"),
        ("archive.tar.gz", "archive.tar (1).gz"),
        ("报告.txt", "报告 (1).txt"),
        ("a (12a).txt", "a (12a) (1).txt"),
    ] {
        let target = directory.path().join(name);
        std::fs::write(&target, b"existing").unwrap();
        assert_eq!(
            unique_sibling(&target).unwrap(),
            directory.path().join(expected)
        );
    }
}

#[test]
fn reservations_are_shared_with_filesystem_collisions_and_count_as_occupied_once() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("a.txt");
    let reserved = directory.path().join("a (1).txt");
    assert_eq!(
        unique_sibling_excluding(&original, |path| path == original || path == reserved).unwrap(),
        directory.path().join("a (2).txt")
    );
}

#[test]
fn directory_names_follow_the_same_rule() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("folder");
    std::fs::create_dir(&target).unwrap();
    assert_eq!(
        unique_sibling(&target).unwrap(),
        directory.path().join("folder (1)")
    );
}

#[cfg(unix)]
#[test]
fn dangling_symlinks_are_occupied_and_directory_lookup_errors_are_reported() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("link");
    std::os::unix::fs::symlink(directory.path().join("missing"), &target).unwrap();
    assert_eq!(
        unique_sibling(&target).unwrap(),
        directory.path().join("link (1)")
    );
    let file = directory.path().join("file");
    std::fs::write(&file, b"data").unwrap();
    assert!(unique_sibling(&file.join("child.txt")).is_err());
}
