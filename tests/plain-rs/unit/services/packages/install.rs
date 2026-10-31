use super::super::manifest::tests::apk;
use super::*;
use std::io::{Cursor, Write};

fn profile() -> splits::Profile {
    splits::Profile {
        abi_order: vec!["arm64-v8a".into()],
        density_dpi: 480,
        locale_tags: vec!["en-US".into()],
    }
}
fn bundle(entries: &[(&str, Vec<u8>)]) -> Cursor<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    Cursor::new(writer.finish().unwrap().into_inner())
}
#[test]
fn bundle_selection_uses_rust_metadata_and_keeps_staging_until_consumed() {
    let cache = tempfile::tempdir().unwrap();
    let archive = bundle(&[
        ("base.apk", apk("com.example.package", None)),
        (
            "split_config.arm64_v8a.apk",
            apk("com.example.package", Some("config.arm64_v8a")),
        ),
        (
            "split_config.x86.apk",
            apk("com.example.package", Some("config.x86")),
        ),
        (
            "split_config.en.apk",
            apk("com.example.package", Some("config.en")),
        ),
        ("standalones/base-x86.apk", apk("com.example.package", None)),
    ]);
    let prepared = prepare(archive, "apks", cache.path(), None, Some(profile())).unwrap();
    assert_eq!(prepared.package_name, "com.example.package");
    assert!(prepared.session);
    assert_eq!(prepared.files.len(), 3);
    assert!(prepared.files.iter().all(|file| file.is_file()));
    let directory = prepared._staging.as_ref().unwrap().path().to_owned();
    drop(prepared);
    assert!(!directory.exists());
}
#[test]
fn standalone_and_nonlocal_single_apks_use_a_session_before_temporary_cleanup() {
    let cache = tempfile::tempdir().unwrap();
    let archive = bundle(&[
        ("standalones/base-x86.apk", apk("com.example.other", None)),
        (
            "standalones/base-arm64-v8a.apk",
            apk("com.example.package", None),
        ),
    ]);
    let prepared = prepare(archive, "apkm", cache.path(), None, Some(profile())).unwrap();
    assert_eq!(prepared.package_name, "com.example.package");
    assert!(prepared.session);
    assert_eq!(prepared.files.len(), 1);
    let prepared = prepare(
        Cursor::new(apk("com.example.package", None)),
        "apk",
        cache.path(),
        None,
        None,
    )
    .unwrap();
    assert!(prepared.session && prepared.files[0].is_file());
    assert_eq!(
        manifest::read(File::open(&prepared.files[0]).unwrap())
            .unwrap()
            .package_name,
        "com.example.package"
    );
}
#[test]
fn ordinary_local_apk_reuses_its_source_without_staging() {
    let cache = tempfile::tempdir().unwrap();
    let path = cache.path().join("source.apk");
    std::fs::write(&path, apk("com.example.package", None)).unwrap();
    let prepared = prepare(
        File::open(&path).unwrap(),
        "apk",
        cache.path(),
        Some(path.clone()),
        None,
    )
    .unwrap();
    assert_eq!(prepared.files, [path]);
    assert!(!prepared.session);
    assert!(prepared._staging.is_none());
    assert!(!cache.path().join("packages").exists());
}
#[test]
fn unsafe_names_mismatched_splits_and_empty_bundles_fail_and_remove_staging() {
    let cache = tempfile::tempdir().unwrap();
    for archive in [
        bundle(&[("../base.apk", apk("com.example.package", None))]),
        bundle(&[
            ("base.apk", apk("com.example.package", None)),
            (
                "split_config.arm64_v8a.apk",
                apk("com.example.other", Some("config.arm64_v8a")),
            ),
        ]),
        bundle(&[("readme.txt", vec![0])]),
        bundle(&[("base.apk", vec![0])]),
    ] {
        assert!(prepare(archive, "apks", cache.path(), None, Some(profile())).is_err());
        assert_eq!(
            std::fs::read_dir(cache.path().join("packages"))
                .unwrap()
                .count(),
            0
        );
    }
}
