use super::*;

fn vector() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../plain-rs/testdata/crypto/web-login-channel-vector.json"
    ))
    .unwrap()
}

fn assert_browser_vector(vector: serde_json::Value, mode: ConfirmationMode) {
    let hello: ServerHello = serde_json::from_value(vector["hello"].clone()).unwrap();
    let reveal: ClientReveal = serde_json::from_value(vector["reveal"].clone()).unwrap();
    let client = Client {
        mode,
        client_id: hello.client_id.clone(),
        key: EcdhSession::from_private_key(&base64_decode(
            vector["clientPrivate"].as_str().unwrap(),
        ))
        .unwrap(),
        nonce: reveal.nonce.clone(),
    };
    assert_eq!(
        reveal.commitment(&hello.client_id, mode.domain()).unwrap(),
        vector["clientHello"]["commitment"]
    );
    assert_eq!(
        hello.signed_transcript(mode.domain()).unwrap(),
        vector["signedTranscript"].as_str().unwrap().as_bytes()
    );
    assert_eq!(
        hello.transcript(&reveal, mode.domain()).unwrap(),
        vector["transcript"].as_str().unwrap().as_bytes()
    );
    let server = ServerHandshake {
        mode,
        key: EcdhSession::from_private_key(&base64_decode(
            vector["serverPrivate"].as_str().unwrap(),
        ))
        .unwrap(),
        hello: hello.clone(),
    }
    .finish(&reveal, 1000)
    .unwrap();
    let (channel, _) = client
        .accept(
            &hello,
            vector["signingPublicKey"].as_str().unwrap(),
            "fixture-device",
            1000,
        )
        .unwrap();
    assert_eq!(channel.context, vector["context"].as_str().unwrap());
    assert_eq!(
        channel.verification_code,
        vector["verificationCode"].as_str().unwrap()
    );
    if mode == ConfirmationMode::DeviceConfirmation {
        assert!(channel.verification_code.starts_with('0'));
    }
    assert_eq!(channel.verification_code, server.verification_code);
    assert_eq!(
        base64_encode(&channel.request_key),
        vector["requestKey"].as_str().unwrap()
    );
    assert_eq!(
        base64_encode(&channel.response_key),
        vector["responseKey"].as_str().unwrap()
    );
    let frame = base64_decode(vector["requestFrame"].as_str().unwrap());
    let decrypted = server.decrypt_request(&frame, 1000).unwrap();
    let request = if mode == ConfirmationMode::Password {
        let digest = vector["passwordHash"].as_str().unwrap();
        assert!(xchacha_decrypt_raw(&digest.as_bytes()[..32], &frame).is_none());
        assert_eq!(
            decrypted,
            base64_decode(vector["passwordFrame"].as_str().unwrap())
        );
        let response = channel
            .decrypt_response(
                &base64_decode(vector["responseFrame"].as_str().unwrap()),
                1000,
            )
            .unwrap();
        assert_eq!(
            xchacha_decrypt_raw(&digest.as_bytes()[..32], &response).unwrap(),
            vector["response"].as_str().unwrap().as_bytes(),
        );
        xchacha_decrypt_raw(&digest.as_bytes()[..32], &decrypted).unwrap()
    } else {
        decrypted
    };
    assert_eq!(request, vector["request"].as_str().unwrap().as_bytes());
}

#[test]
fn browser_vector_binds_commitment_signature_transcript_keys_and_ciphertext_in_rust() {
    assert_browser_vector(vector(), ConfirmationMode::DeviceConfirmation);
}

#[test]
fn password_browser_vector_requires_both_encryption_layers_and_agrees_with_rust() {
    let vector = serde_json::from_str(include_str!(
        "../../../../plain-rs/testdata/crypto/web-password-login-vector.json"
    ))
    .unwrap();
    assert_browser_vector(vector, ConfirmationMode::Password);
}

fn pair() -> (Channel, Channel) {
    let (private, public) = plain_rs::crypto::ed25519_generate();
    let (client, hello) = Client::start("browser", ConfirmationMode::DeviceConfirmation).unwrap();
    let (server, response) = ServerHandshake::start(
        "browser",
        ConfirmationMode::DeviceConfirmation,
        "device",
        &hello,
        &private,
        1000,
    )
    .unwrap();
    let (client, reveal) = client
        .accept(&response, &base64_encode(&public), "device", 1000)
        .unwrap();
    (client, server.finish(&reveal, 1000).unwrap())
}

#[test]
fn derivation_matches_rfc5869_sha256_case_one() {
    let key = derive(
        &[0x0b; 22],
        &(0_u8..=12).collect::<Vec<_>>(),
        &(0xf0_u8..=0xf9).collect::<Vec<_>>(),
    );
    assert_eq!(
        plain_rs::utils::hex::bytes_to_hex(&key),
        "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf"
    );
}

#[test]
fn actual_committed_exchange_agrees_and_separates_directions() {
    let (client, server) = pair();
    assert_eq!(client.context, server.context);
    assert_eq!(client.verification_code, server.verification_code);
    let bytes = client.verification_code.as_bytes();
    assert_eq!(bytes.len(), 9);
    assert_eq!(bytes[4], b' ');
    assert!(
        bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || byte.is_ascii_digit())
    );
    let request = client.encrypt_request(b"private metadata", 1000).unwrap();
    assert_eq!(
        server.decrypt_request(&request, 1000).unwrap(),
        b"private metadata"
    );
    assert!(client.decrypt_response(&request, 1000).is_err());
    let response = server.encrypt_response(b"approved", 1000).unwrap();
    assert_eq!(
        client.decrypt_response(&response, 1000).unwrap(),
        b"approved"
    );
    assert!(server.decrypt_request(&response, 1000).is_err());
    let (_, other) = pair();
    assert!(other.decrypt_request(&request, 1000).is_err());
    assert!(other.decrypt_response(&response, 1000).is_err());
}

#[test]
fn encrypted_frames_expire_at_the_advertised_boundary() {
    let (client, server) = pair();
    let request = client.encrypt_request(b"request", 300_999).unwrap();
    assert!(server.decrypt_request(&request, 300_999).is_ok());
    assert!(server.decrypt_request(&request, 301_000).is_err());
    assert!(client.encrypt_request(b"request", 301_000).is_err());
    assert!(server.encrypt_response(b"response", 301_000).is_err());
    let response = server.encrypt_response(b"response", 1000).unwrap();
    assert!(client.decrypt_response(&response, 301_000).is_err());
}

#[test]
fn every_signed_field_and_signature_reject_substitution() {
    for field in [
        "clientId",
        "deviceId",
        "clientCommitment",
        "serverPublicKey",
        "serverNonce",
        "expiresAt",
        "signature",
    ] {
        let (private, public) = plain_rs::crypto::ed25519_generate();
        let (client, hello) =
            Client::start("browser", ConfirmationMode::DeviceConfirmation).unwrap();
        let (_, response) = ServerHandshake::start(
            "browser",
            ConfirmationMode::DeviceConfirmation,
            "device",
            &hello,
            &private,
            1000,
        )
        .unwrap();
        let mut value = serde_json::to_value(response).unwrap();
        value[field] = match field {
            "expiresAt" => serde_json::json!(301_001),
            "serverPublicKey" => {
                serde_json::json!(base64_encode(&EcdhSession::generate().public_key_bytes))
            }
            "clientCommitment" | "serverNonce" => serde_json::json!(nonce()),
            _ => serde_json::json!("substituted"),
        };
        assert!(
            client
                .accept(
                    &serde_json::from_value(value).unwrap(),
                    &base64_encode(&public),
                    "",
                    1000
                )
                .is_err(),
            "{field}"
        );
    }
}

#[test]
fn reveal_cannot_change_committed_key_nonce_or_client_identity() {
    for field in ["publicKey", "nonce", "clientId"] {
        let (private, _) = plain_rs::crypto::ed25519_generate();
        let (client, hello) =
            Client::start("browser", ConfirmationMode::DeviceConfirmation).unwrap();
        let reveal = client.reveal();
        let id = if field == "clientId" {
            "other-browser"
        } else {
            "browser"
        };
        let (server, _) = ServerHandshake::start(
            id,
            ConfirmationMode::DeviceConfirmation,
            "device",
            &hello,
            &private,
            1000,
        )
        .unwrap();
        let mut value = serde_json::to_value(reveal).unwrap();
        if field == "publicKey" {
            value[field] =
                serde_json::json!(base64_encode(&EcdhSession::generate().public_key_bytes));
        }
        if field == "nonce" {
            value[field] = serde_json::json!(nonce());
        }
        assert!(
            server
                .finish(&serde_json::from_value(value).unwrap(), 1000)
                .is_err(),
            "{field}"
        );
    }
}

#[test]
fn commitment_is_hiding_and_legacy_or_extra_hello_fields_are_rejected() {
    let (_, hello) = Client::start("browser", ConfirmationMode::DeviceConfirmation).unwrap();
    let value = serde_json::to_value(hello).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 1);
    assert!(value.get("commitment").is_some());
    assert!(
        serde_json::from_value::<ClientHello>(
            serde_json::json!({"publicKey":"key", "nonce":"nonce", "protocol":"plain-web-login-v1"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<ClientHello>(
            serde_json::json!({"commitment":nonce(), "protocol":"ignored"})
        )
        .is_err()
    );
    let (private, _) = plain_rs::crypto::ed25519_generate();
    for commitment in [
        String::new(),
        base64_encode(&[0; 31]),
        format!("{}\n", nonce()),
    ] {
        assert!(
            ServerHandshake::start(
                "browser",
                ConfirmationMode::DeviceConfirmation,
                "device",
                &ClientHello { commitment },
                &private,
                1000
            )
            .is_err()
        );
    }
}

#[test]
fn trusted_identity_and_fresh_commitment_cannot_be_replaced() {
    let (private, public) = plain_rs::crypto::ed25519_generate();
    let (client, hello) = Client::start("browser", ConfirmationMode::DeviceConfirmation).unwrap();
    let (_, response) = ServerHandshake::start(
        "browser",
        ConfirmationMode::DeviceConfirmation,
        "device",
        &hello,
        &private,
        1000,
    )
    .unwrap();
    let (other, _) = Client::start("browser", ConfirmationMode::DeviceConfirmation).unwrap();
    assert!(
        other
            .accept(&response, &base64_encode(&public), "device", 1000)
            .is_err()
    );
    assert!(
        client
            .accept(&response, &base64_encode(&public), "other-device", 1000)
            .is_err()
    );
    let (client, hello) = Client::start("browser", ConfirmationMode::DeviceConfirmation).unwrap();
    let (_, response) = ServerHandshake::start(
        "browser",
        ConfirmationMode::DeviceConfirmation,
        "device",
        &hello,
        &private,
        1000,
    )
    .unwrap();
    let (_, replacement) = plain_rs::crypto::ed25519_generate();
    assert!(
        client
            .accept(&response, &base64_encode(&replacement), "device", 1000)
            .is_err()
    );
}

#[test]
fn reveal_and_signed_hello_expire_and_client_clock_cannot_extend_lifetime() {
    let (private, public) = plain_rs::crypto::ed25519_generate();
    let (client, hello) = Client::start("browser", ConfirmationMode::DeviceConfirmation).unwrap();
    let (server, response) = ServerHandshake::start(
        "browser",
        ConfirmationMode::DeviceConfirmation,
        "device",
        &hello,
        &private,
        1000,
    )
    .unwrap();
    let (channel, reveal) = client
        .accept(&response, &base64_encode(&public), "device", 900)
        .unwrap();
    assert!(server.finish(&reveal, 301_000).is_err());
    assert!(channel.encrypt_request(b"request", 300_899).is_ok());
    assert!(channel.encrypt_request(b"request", 300_900).is_err());
    let (client, hello) = Client::start("browser", ConfirmationMode::DeviceConfirmation).unwrap();
    let (_, response) = ServerHandshake::start(
        "browser",
        ConfirmationMode::DeviceConfirmation,
        "device",
        &hello,
        &private,
        1000,
    )
    .unwrap();
    assert!(
        client
            .accept(&response, &base64_encode(&public), "device", 301_000)
            .is_err()
    );
}

#[test]
fn server_rejects_invalid_committed_points_nonces_and_identities() {
    let (private, _) = plain_rs::crypto::ed25519_generate();
    for (public_key, nonce) in [
        (base64_encode(&[0; 65]), nonce()),
        (
            base64_encode(&EcdhSession::generate().public_key_bytes),
            base64_encode(&[0; 31]),
        ),
    ] {
        let reveal = ClientReveal { public_key, nonce };
        let hello = ClientHello {
            commitment: reveal
                .commitment("browser", ConfirmationMode::DeviceConfirmation.domain())
                .unwrap(),
        };
        let (server, _) = ServerHandshake::start(
            "browser",
            ConfirmationMode::DeviceConfirmation,
            "device",
            &hello,
            &private,
            1000,
        )
        .unwrap();
        assert!(server.finish(&reveal, 1000).is_err());
    }
    let (_, hello) = Client::start("browser", ConfirmationMode::DeviceConfirmation).unwrap();
    assert!(
        ServerHandshake::start(
            "",
            ConfirmationMode::DeviceConfirmation,
            "device",
            &hello,
            &private,
            1000
        )
        .is_err()
    );
    assert!(
        ServerHandshake::start(
            "browser",
            ConfirmationMode::DeviceConfirmation,
            "",
            &hello,
            &private,
            1000
        )
        .is_err()
    );
    assert!(
        ServerHandshake::start(
            "browser",
            ConfirmationMode::DeviceConfirmation,
            "device",
            &hello,
            &[0; 4],
            1000
        )
        .is_err()
    );
}

#[test]
fn confirmation_domains_reject_cross_protocol_challenges() {
    let (private, public) = plain_rs::crypto::ed25519_generate();
    for (client_mode, server_mode) in [
        (
            ConfirmationMode::ChatPairing,
            ConfirmationMode::DeviceConfirmation,
        ),
        (
            ConfirmationMode::DeviceConfirmation,
            ConfirmationMode::ChatPairing,
        ),
        (ConfirmationMode::ChatPairing, ConfirmationMode::Password),
        (ConfirmationMode::Password, ConfirmationMode::ChatPairing),
        (
            ConfirmationMode::Password,
            ConfirmationMode::DeviceConfirmation,
        ),
        (
            ConfirmationMode::DeviceConfirmation,
            ConfirmationMode::Password,
        ),
    ] {
        let (client, hello) = Client::start("browser", client_mode).unwrap();
        let (_, response) =
            ServerHandshake::start("browser", server_mode, "device", &hello, &private, 1000)
                .unwrap();
        assert!(
            client
                .accept(&response, &base64_encode(&public), "device", 1000)
                .is_err()
        );
    }
}
