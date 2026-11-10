use super::*;
#[test]
fn authenticated_completion_requires_expected_identity_and_pair_permission() {
    let mut response = json!({"clientId":"peer","status":"COMPLETED","chatPaired":true});
    assert_eq!(verify_response(&response, "peer").unwrap(), "peer");
    assert!(verify_response(&response, "other").is_err());
    response["chatPaired"] = json!(false);
    assert!(verify_response(&response, "peer").is_err());
    response["chatPaired"] = json!(true);
    response["status"] = json!("PENDING");
    assert!(verify_response(&response, "peer").is_err());
    assert!(check_identity("peer", "known", "peer", "replacement").is_err());
}
#[test]
fn password_completion_rejects_reflection_and_wrong_key() {
    let context = plain_libs::auth::context("client", &[1; 32]);
    let key = [2; 32];
    let plain = br#"{"status":"COMPLETED"}"#;
    let p = Protection::Password {
        key,
        aad: plain_libs::auth::auth_aad(&context, true),
    };
    let response = plain_libs::crypto::xchacha_encrypt_raw_with_aad(
        &key,
        plain,
        &plain_libs::auth::auth_aad(&context, true),
    )
    .unwrap();
    assert_eq!(p.decrypt(&response).unwrap(), plain);
    let reflection = plain_libs::crypto::xchacha_encrypt_raw_with_aad(
        &key,
        plain,
        &plain_libs::auth::auth_aad(&context, false),
    )
    .unwrap();
    assert!(p.decrypt(&reflection).is_err());
    let wrong = Protection::Password {
        key: [3; 32],
        aad: plain_libs::auth::auth_aad(&context, true),
    };
    assert!(wrong.decrypt(&response).is_err());
}
