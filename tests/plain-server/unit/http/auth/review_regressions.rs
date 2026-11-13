use super::*;

#[test]
fn malformed_peer_metadata_is_rejected_instead_of_skipping_pairing() {
    let metadata = json!({"appName":"Fixture","appVersion":"1","osName":"Linux","osVersion":"1"});
    assert!(validate_metadata(&metadata).is_ok());
    let mut paired = metadata.clone();
    paired["peer"] = json!({"deviceName":"Fixture","signaturePublicKey":plain_libs::utils::base64::base64_encode(&[7;32]),"port":8080,"deviceType":"PHONE","ips":["127.0.0.1"]});
    assert!(validate_metadata(&paired).is_ok());
    let mut paired = metadata.clone();
    paired["peer"] = json!({"deviceName":"Fixture","signaturePublicKey":plain_libs::utils::base64::base64_encode(&[7;32]),"port":8080,"deviceType":"PHONE","ips":["127.0.0.1"]});
    assert!(validate_metadata(&paired).is_ok());
    for peer in [
        json!({"deviceName":"Fixture","signaturePublicKey":"bad","port":8080,"deviceType":"PHONE","ips":[]}),
        json!({"deviceName":"Fixture","signaturePublicKey":plain_libs::utils::base64::base64_encode(&[7;32]),"port":0,"deviceType":"PHONE","ips":[]}),
        json!({"deviceName":"Fixture","signaturePublicKey":plain_libs::utils::base64::base64_encode(&[7;32]),"port":-1,"deviceType":"PHONE","ips":[]}),
    ] {
        let mut invalid = metadata.clone();
        invalid["peer"] = peer;
        assert!(validate_metadata(&invalid).is_err());
    }
}

#[test]
fn login_errors_keep_storage_failures_distinct_from_credentials() {
    let invalid = plain_libs::anyhow::anyhow!("invalid_password");
    assert_eq!(
        LoginError::from_error(&invalid),
        LoginError::InvalidPassword
    );
    assert_eq!(LoginError::InvalidPassword.close_code(), 1008);
    let storage = plain_libs::anyhow::anyhow!("fixture disk failure");
    assert_eq!(LoginError::from_error(&storage), LoginError::Internal);
    assert_eq!(LoginError::Internal.close_code(), 1011);
    assert_eq!(LoginError::RateLimited.close_code(), 1013);
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
        matches!(login_frame(&mut socket).await, Message::Close(Some(frame)) if frame.code == 1008.into() && frame.reason == "invalid_login_request")
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
