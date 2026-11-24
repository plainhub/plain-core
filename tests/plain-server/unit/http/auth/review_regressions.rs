use plain_rs::features::services::auth_error::AuthErrorCode;
use super::*;

#[test]
fn malformed_peer_metadata_is_rejected_instead_of_skipping_pairing() {
    let metadata = json!({"appName":"Fixture","appVersion":"1","osName":"Linux","osVersion":"1"});
    assert!(validate_metadata(&metadata).is_ok());
    let peer = json!({"deviceName":"Fixture","signaturePublicKey":plain_libs::utils::base64::base64_encode(&[7;32]),
        "encryptionPublicKey":plain_libs::utils::base64::base64_encode(&[8;32]),"port":8080,"deviceType":"PHONE","ips":["127.0.0.1"]});
    let mut paired = metadata;
    paired["peer"] = peer.clone();
    assert!(validate_metadata(&paired).is_ok());
    for (key, invalid) in [
        ("signaturePublicKey", json!("bad")),
        ("encryptionPublicKey", json!("bad")),
        ("encryptionPublicKey", Value::Null),
        ("port", json!(0)),
        ("port", json!(-1)),
    ] {
        paired["peer"] = peer.clone();
        paired["peer"][key] = invalid;
        assert!(validate_metadata(&paired).is_err(), "{key}");
    }
}

#[test]
fn login_errors_keep_storage_failures_distinct_from_credentials() {
    let invalid = plain_libs::anyhow::anyhow!(AuthErrorCode::InvalidPassword);
    assert_eq!(
        login_error(&invalid),
        AuthErrorCode::InvalidPassword
    );
    assert_eq!(login_close_code(AuthErrorCode::InvalidPassword), 1008);
    let storage = plain_libs::anyhow::anyhow!("fixture disk failure");
    assert_eq!(login_error(&storage), AuthErrorCode::InternalError);
    assert_eq!(login_close_code(AuthErrorCode::InternalError), 1011);
    assert_eq!(login_close_code(AuthErrorCode::TooManyLoginAttempts), 1013);
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn auth_rejects_non_uuid_client_id_before_handshake() {
    use plain_libs::tokio_tungstenite::tungstenite::Message;
    let (_directory, server, port) = passwordless_server().await;
    let (mut socket, _) = plain_libs::tokio_tungstenite::connect_async(format!(
        "ws://127.0.0.1:{port}/auth?cid=wasm-client"
    ))
    .await
    .unwrap();
    assert!(
        matches!(login_frame(&mut socket).await, Message::Close(Some(frame)) if frame.code == 1008.into() && frame.reason == "INVALID_LOGIN_REQUEST")
    );
    assert!(
        server
            .runtime_state()
            .db
            .session_get("wasm-client")
            .unwrap()
            .is_none()
    );
    server.shutdown().await;
}

#[test]
fn login_error_classification_preserves_contextual_failure() {
    let failure = plain_libs::anyhow::anyhow!(AuthErrorCode::InvalidPassword).context("login request failed");
    assert_eq!(login_error(&failure), AuthErrorCode::InvalidPassword);
}
