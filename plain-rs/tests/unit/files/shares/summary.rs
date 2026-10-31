use super::*;

fn root(path: &std::path::Path) -> Root {
    Root {
        relative_path: "root".into(),
        real_path: path.to_str().unwrap().into(),
        is_dir: path.is_dir(),
    }
}

#[test]
fn single_directory_counts_visible_children_without_walking_directories() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("visible"), b"abc").unwrap();
    fs::write(dir.path().join(".hidden"), b"hidden").unwrap();
    fs::create_dir(dir.path().join("nested")).unwrap();
    fs::write(dir.path().join("nested/child"), b"not counted").unwrap();
    assert_eq!(
        summarize(&[root(dir.path())]).unwrap(),
        Summary {
            item_count: 2,
            total_size: 3
        }
    );
}

#[test]
fn multiple_roots_count_files_and_directories_without_expanding_them() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("nested");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("child"), b"not counted").unwrap();
    let file = dir.path().join(".hidden-root");
    fs::write(&file, b"bytes").unwrap();
    assert_eq!(
        summarize(&[root(&nested), root(&file)]).unwrap(),
        Summary {
            item_count: 2,
            total_size: 5
        }
    );
}

#[test]
fn large_files_preserve_64_bit_sizes_and_missing_roots_count_without_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("large");
    fs::File::create(&file)
        .unwrap()
        .set_len(3 * 1024 * 1024 * 1024)
        .unwrap();
    assert_eq!(
        summarize(&[root(&file)]).unwrap(),
        Summary {
            item_count: 1,
            total_size: 3 * 1024 * 1024 * 1024
        }
    );
    fs::remove_file(&file).unwrap();
    assert_eq!(
        summarize(&[root(&file)]).unwrap(),
        Summary {
            item_count: 1,
            total_size: 0
        }
    );
}
