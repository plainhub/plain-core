use super::*;

#[test]
fn batching_preserves_order_and_resolves_each_distinct_value_once_including_failures() {
    let mut calls = Vec::new();
    let values = resolve_many(
        vec![
            "app://first".into(),
            "missing".into(),
            "app://first".into(),
            "missing".into(),
            "last".into(),
        ],
        |path| {
            calls.push(path.to_owned());
            (path != "missing").then(|| format!("resolved:{path}"))
        },
    );
    assert_eq!(calls, ["app://first", "missing", "last"]);
    assert_eq!(
        values,
        [
            Some("resolved:app://first".into()),
            None,
            Some("resolved:app://first".into()),
            None,
            Some("resolved:last".into())
        ]
    );
}
#[test]
fn aliases_use_the_owned_data_directory_and_reject_escapes_without_a_host() {
    let directory = tempfile::tempdir().unwrap();
    let db = std::sync::Arc::new(plain_rs::db::Db::open(Path::new(":memory:")).unwrap());
    assert_eq!(
        Path::new(
            &resolve(
                db.clone(),
                directory.path().to_path_buf(),
                "app://icons/photo.png"
            )
            .unwrap()
        ),
        directory.path().join("icons/photo.png")
    );
    for value in [
        "app://../outside",
        "app:///outside",
        "app://a/../../outside",
        "app://a\\outside",
    ] {
        assert!(
            resolve(db.clone(), directory.path().to_path_buf(), value).is_err(),
            "{value}"
        );
    }
    assert!(resolve(db, PathBuf::from("relative"), "app://photo").is_err());
}

#[test]
fn ordinary_relative_references_use_the_core_directory_instead_of_the_process_directory() {
    let directory = tempfile::tempdir().unwrap();
    let db = std::sync::Arc::new(plain_rs::db::Db::open(Path::new(":memory:")).unwrap());
    assert_eq!(
        Path::new(
            &resolve(
                db.clone(),
                directory.path().to_path_buf(),
                "documents/file.txt"
            )
            .unwrap()
        ),
        directory.path().join("documents/file.txt")
    );
    assert!(resolve(db.clone(), PathBuf::from("relative"), "documents/file.txt").is_err());
    for value in ["content://media/item/1", "ph://photo"] {
        assert_eq!(
            resolve(db.clone(), directory.path().to_path_buf(), value).unwrap(),
            value
        );
    }
}
