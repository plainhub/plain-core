use super::*;

#[test]
fn identity_receipts_reject_empty_ids_duplicate_paths_foreign_paths_and_non_media_types() {
    let paths = vec!["/media/image.png".to_owned()];
    let row = Identity {
        media_type: 3,
        file_id: "42".into(),
        path: paths[0].clone(),
    };
    validate(&[row.clone()], &paths).unwrap();
    for rows in [
        vec![Identity {
            file_id: String::new(),
            ..row.clone()
        }],
        vec![row.clone(), row.clone()],
        vec![Identity {
            path: "/media/foreign.png".into(),
            ..row.clone()
        }],
        vec![Identity {
            media_type: 6,
            ..row.clone()
        }],
    ] {
        assert!(validate(&rows, &paths).is_err());
    }
}
