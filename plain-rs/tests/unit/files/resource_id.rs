use super::*;

#[test]
fn extract_path_accepts_media_item_envelope() {
    assert_eq!(
        extract_path(r#"{"path":"/x/b.svg","mediaId":"87772750-b50d-5f1b"}"#),
        "/x/b.svg"
    );
    assert_eq!(extract_path("/x/b.svg"), "/x/b.svg");
    assert_eq!(extract_path("/x/plain file.svg"), "/x/plain file.svg");
    // A JSON doc without a string "path" is not an envelope: pass through
    // (the fs handler will 404 on stat, same as any bogus path).
    assert_eq!(
        extract_path(r#"{"file":"/x/b.svg"}"#),
        r#"{"file":"/x/b.svg"}"#
    );
}

#[test]
fn path_from_file_id_roundtrips_both_shapes() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap();
    let token = "dG9rZW5fdG9rZW5fdG9rZW5fdG9rZW5fdG9rZW5fdG8="; // 32 bytes
    prefs.set("url_token", token).unwrap();
    let key = base64_decode(token).unwrap();

    let raw = plain_rs::crypto::xchacha_encrypt_raw(&key, b"/x/a.jpg").unwrap();
    let id_raw = plain_rs::utils::base64::base64_encode(&raw);
    assert_eq!(
        path_from_file_id(&id_raw, &prefs).unwrap(),
        "/x/a.jpg",
        "bare-path ids keep working"
    );

    let envelope = format!(
        r#"{{"path":"/home/smartbox/memory/plainapp/design/b.svg","mediaId":"87772750-b50d-5f1b-912e-19614bf7d366"}}"#
    );
    let enc = plain_rs::crypto::xchacha_encrypt_raw(&key, envelope.as_bytes()).unwrap();
    let id_env = plain_rs::utils::base64::base64_encode(&enc);
    assert_eq!(
        path_from_file_id(&id_env, &prefs).unwrap(),
        "/home/smartbox/memory/plainapp/design/b.svg",
        "plain-app media-item envelope ids resolve to their path"
    );
}
