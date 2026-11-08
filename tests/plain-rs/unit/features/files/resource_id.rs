use super::*;

#[test]
fn path_from_file_id_accepts_typed_global_paths_and_rejects_other_scopes() {
    use crate::features::files::resource_link::{self, Resource, Scope};
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = crate::features::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap();
    prefs
        .set(
            "url_token",
            plain_libs::utils::base64::base64_encode(&[7; 32]),
        )
        .unwrap();
    let id = resource_link::seal(
        &Scope::Global,
        &[7; 32],
        &Resource::reference("/photos/preview.svg"),
    )
    .unwrap();
    assert_eq!(
        path_from_file_id(&id, &prefs).unwrap(),
        "/photos/preview.svg"
    );
    let id = resource_link::seal(
        &Scope::Session("client".into()),
        &[7; 32],
        &Resource::reference("/file"),
    )
    .unwrap();
    assert!(path_from_file_id(&id, &prefs).is_err());
    assert!(path_from_file_id("oldBase64", &prefs).is_err());
}
