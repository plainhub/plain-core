use super::*;
#[test]
fn signed_exchange_binds_identity_client_request_and_direction() {
    let (signing, public) = plain_libs::crypto::ed25519_generate();
    let (client, request) = Client::start("browser").unwrap();
    let (server, response) =
        ServerHandshake::start("browser", "device", &request, &signing, 100).unwrap();
    let client = client
        .accept(&response, &base64_encode(&public), "device", 101)
        .unwrap();
    assert_eq!(client.api_key(), server.api_key());
    assert_eq!(client.context, server.context);
    assert_eq!(client.verification_code.len(), 64);
    let metadata = client.encrypt_request(b"metadata", 102).unwrap();
    assert_eq!(server.decrypt_request(&metadata, 102).unwrap(), b"metadata");
    assert!(server.decrypt_response(&metadata, 102).is_err());
    let mut tampered = metadata.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    assert!(server.decrypt_request(&tampered, 102).is_err());
    let result = server.encrypt_response(b"result", 102).unwrap();
    assert_eq!(client.decrypt_response(&result, 102).unwrap(), b"result");
    assert!(client.decrypt_response(&result, 300100).is_err());
}
#[test]
fn altered_client_key_cid_identity_signature_and_expiry_are_rejected() {
    let (signing, public) = plain_libs::crypto::ed25519_generate();
    for case in 0..5 {
        let (client, mut request) = Client::start("browser").unwrap();
        if case == 0 {
            request.public_key = base64_encode(&EcdhSession::generate().public_key_bytes);
        }
        let (_, mut response) = ServerHandshake::start(
            if case == 1 { "other" } else { "browser" },
            "device",
            &request,
            &signing,
            100,
        )
        .unwrap();
        if case == 2 {
            response.device_id = "other".into();
        }
        if case == 3 {
            response.signature = base64_encode(&[0; 64]);
        }
        let result = client.accept(
            &response,
            &base64_encode(&public),
            "device",
            if case == 4 { 300100 } else { 101 },
        );
        assert!(result.is_err(), "case {case}");
    }
}
#[test]
fn independent_exchanges_have_distinct_keys_and_fingerprints() {
    let (signing, _) = plain_libs::crypto::ed25519_generate();
    let (_, request) = Client::start("browser").unwrap();
    let (a, _) = ServerHandshake::start("browser", "device", &request, &signing, 100).unwrap();
    let (b, _) = ServerHandshake::start("browser", "device", &request, &signing, 100).unwrap();
    assert_ne!(a.api_key(), b.api_key());
    assert_ne!(a.context, b.context);
}
#[test]
fn exports_browser_interop_vector() {
    let (signing, public) = plain_libs::crypto::ed25519_generate();
    let (client, request) = Client::start("fixture-browser").unwrap();
    let private = client.key.private_key_bytes();
    let (server, response) = ServerHandshake::start(
        "fixture-browser",
        "fixture-device",
        &request,
        &signing,
        1700000000000,
    )
    .unwrap();
    let verified = client
        .accept(
            &response,
            &base64_encode(&public),
            "fixture-device",
            1700000000001,
        )
        .unwrap();
    assert_eq!(verified.api_key(), server.api_key());
    if let Ok(path) = std::env::var("PLAIN_WEB_APPROVAL_VECTOR") {
        let vector = json!({"signedPayload":String::from_utf8(signed_bytes("fixture-browser", &base64_encode(&public), &request, &response)).unwrap(),"clientId":"fixture-browser","clientPrivate":base64_encode(&private),"serverIdentity":base64_encode(&public),"request":request,"response":response,"fingerprint":server.context,"key":base64_encode(&server.api_key())});
        std::fs::write(path, plain_libs::serde_json::to_vec_pretty(&vector).unwrap()).unwrap();
    }
}

#[test]
fn response_signature_has_one_purpose_and_keeps_all_bindings() {
    let (signing, public) = plain_libs::crypto::ed25519_generate();
    let (_, hello) = Client::start("browser").unwrap();
    let (_, response) = ServerHandshake::start("browser", "device", &hello, &signing, 100).unwrap();
    let bytes = signed_bytes("browser", &base64_encode(&public), &hello, &response);
    assert!(ed25519_verify(&base64_encode(&public), &bytes, &response.signature));
    let payload: plain_libs::serde_json::Value = plain_libs::serde_json::from_slice(&bytes).unwrap();
    assert_eq!(payload[0], "WEB_APPROVAL_RESPONSE");
    assert_eq!(payload.as_array().unwrap().len(), 6);
    for index in 0..6 {
        let mut altered = payload.clone();
        altered[index] = json!("substituted");
        assert!(!ed25519_verify(&base64_encode(&public), altered.to_string().as_bytes(), &response.signature));
    }
}
