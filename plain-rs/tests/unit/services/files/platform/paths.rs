use super::*;

#[test]
fn android_write_policy_retains_protected_roots_and_component_boundaries() {
    for path in [
        "/",
        "/sdcard",
        "/storage",
        "/data",
        "/system/bin",
        "/proc/1/maps",
        "/sys/block",
        "/dev/null",
        "/data/data/app",
        "/data/app/package",
        "/vendor/lib",
        "/product/app",
        "/apex/library",
        "/oem/app",
        "/odm/lib",
    ] {
        assert!(validate_android(Path::new(path)).is_err(), "{path}");
    }
    for path in [
        "/storage/emulated/0/file.txt",
        "/sdcard/Download",
        "/systematic/file",
        "/data/user/0/app/file",
    ] {
        assert!(validate_android(Path::new(path)).is_ok(), "{path}");
    }
}

#[test]
fn private_classification_resolves_missing_paths_and_parent_components() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let data = root.join("data");
    let cache = root.join("cache");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let paths = vec![
        data.join("missing/deep/file")
            .to_string_lossy()
            .into_owned(),
        cache.join("item").to_string_lossy().into_owned(),
        root.join("external/file").to_string_lossy().into_owned(),
        data.join("../outside").to_string_lossy().into_owned(),
        root.join("data-other/file").to_string_lossy().into_owned(),
    ];
    assert_eq!(external(&paths, &data, &cache, true).unwrap(), paths[2..]);
    assert!(external(&["relative/file".into()], &data, &cache, false).is_err());
    assert!(external(&["/".into()], &data, &cache, true).is_err());
    assert_eq!(
        canonical(&data.join("missing/../../outside")).unwrap(),
        root.join("outside")
    );
}

#[cfg(unix)]
#[test]
fn symlinks_cannot_disguise_external_paths_as_private_or_bypass_parent_resolution() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let data = root.join("data");
    let cache = root.join("cache");
    let outside = root.join("outside");
    for path in [&data, &cache, &outside] {
        std::fs::create_dir(path).unwrap();
    }
    std::os::unix::fs::symlink(&outside, data.join("escape")).unwrap();
    std::os::unix::fs::symlink(&data, root.join("private-alias")).unwrap();
    std::os::unix::fs::symlink(root.join("missing"), data.join("dangling")).unwrap();
    let path = data.join("escape/missing").to_string_lossy().into_owned();
    assert_eq!(
        external(&[path.clone()], &data, &cache, false).unwrap(),
        vec![path]
    );
    assert!(
        external(
            &[root
                .join("private-alias/file")
                .to_string_lossy()
                .into_owned()],
            &data,
            &cache,
            false
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        canonical(&data.join("escape/../file")).unwrap(),
        root.join("file")
    );
    assert!(
        external(
            &[data.join("dangling/file").to_string_lossy().into_owned()],
            &data,
            &cache,
            false
        )
        .is_err()
    );
}
