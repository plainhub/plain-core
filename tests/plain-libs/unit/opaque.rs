use super::*;
#[path = "auth_rng.rs"]
mod rng;
use crate::serde_json::Value;

fn fixture() -> Value {
    crate::serde_json::from_str(include_str!(
        "../../../plain-rs/testdata/opaque/extraction-reference.json"
    ))
    .unwrap()
}
fn bytes<const N: usize>(fixture: &Value, name: &str) -> [u8; N] {
    rng::decode(fixture[name].as_str().unwrap())
}
fn small_ksf() -> Argon2<'static> {
    Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(32, 1, 1, None).unwrap(),
    )
}
fn record() -> (ServerSetup, ServerRegistration) {
    let f = fixture();
    (
        ServerSetup::deserialize(&bytes::<128>(&f, "setup")).unwrap(),
        ServerRegistration::deserialize(&bytes::<192>(&f, "record")).unwrap(),
    )
}

#[test]
fn existing_reference_setup_records_and_wire_messages_are_byte_identical() {
    let f = fixture();
    let password = f["password"].as_str().unwrap().as_bytes();
    let identity = bytes::<32>(&f, "identity");
    let ksf = crate::auth::ksf();
    let mut source = rng::Stream::new(1);
    let setup = ServerSetup::new(&mut source);
    assert_eq!(setup.serialize(), bytes::<128>(&f, "setup"));
    assert_eq!(source.bytes, 192);
    let mut source = rng::Stream::new(2);
    let client = ClientRegistration::start(&mut source, password).unwrap();
    assert_eq!(
        client.message.serialize(),
        bytes::<32>(&f, "registration_request")
    );
    assert_eq!(source.bytes, 64);
    let response =
        ServerRegistration::start(&setup, client.message, crate::auth::CLIENT_IDENTITY).unwrap();
    assert_eq!(
        response.message.serialize(),
        bytes::<64>(&f, "registration_response")
    );
    let mut source = rng::Stream::new(3);
    let upload = client
        .state
        .finish(
            &mut source,
            password,
            response.message,
            ClientRegistrationFinishParameters::new(
                Identifiers {
                    client: Some(crate::auth::CLIENT_IDENTITY),
                    server: Some(&identity),
                },
                Some(&ksf),
            ),
        )
        .unwrap();
    assert_eq!(upload.message.serialize(), bytes::<192>(&f, "record"));
    assert_eq!(*upload.export_key, bytes::<64>(&f, "export_key"));
    assert_eq!(source.bytes, 32);
    let (setup, record) = record();
    let context = crate::auth::context("synthetic-client", &identity);
    let mut source = rng::Stream::new(4);
    let client = ClientLogin::start(&mut source, password).unwrap();
    assert_eq!(client.message.serialize(), bytes::<96>(&f, "ke1"));
    assert_eq!(source.bytes, 128);
    let mut source = rng::Stream::new(5);
    let server = ServerLogin::start(
        &mut source,
        &setup,
        Some(record),
        client.message,
        crate::auth::CLIENT_IDENTITY,
        ServerLoginParameters {
            context: Some(&context),
            identifiers: Identifiers {
                client: Some(crate::auth::CLIENT_IDENTITY),
                server: Some(&identity),
            },
        },
    )
    .unwrap();
    assert_eq!(server.message.serialize(), bytes::<320>(&f, "ke2"));
    assert_eq!(source.bytes, 160);
    let mut source = rng::Stream::new(6);
    let client = client
        .state
        .finish(
            &mut source,
            password,
            server.message,
            ClientLoginFinishParameters::new(
                Some(&context),
                Identifiers {
                    client: Some(crate::auth::CLIENT_IDENTITY),
                    server: Some(&identity),
                },
                Some(&ksf),
            ),
        )
        .unwrap();
    assert_eq!(source.bytes, 0);
    assert_eq!(client.message.serialize(), bytes::<64>(&f, "ke3"));
    assert_eq!(*client.session_key, bytes::<64>(&f, "session_key"));
    assert_eq!(*client.export_key, bytes::<64>(&f, "export_key"));
    assert_eq!(
        client.server_s_pk.serialize(),
        bytes::<32>(&f, "public_key")
    );
    let server = server
        .state
        .finish(client.message, Default::default())
        .unwrap();
    assert_eq!(*server.session_key, *client.session_key);
    assert_eq!(
        crate::auth::api_key(&server.session_key[..], &context),
        bytes::<32>(&f, "api_key")
    );
}

#[test]
fn truncated_and_invalid_encoded_points_or_private_keys_fail_closed() {
    let f = fixture();
    let setup = bytes::<128>(&f, "setup");
    let record = bytes::<192>(&f, "record");
    let ke1 = bytes::<96>(&f, "ke1");
    let ke2 = bytes::<320>(&f, "ke2");
    for n in 0..128 {
        assert!(ServerSetup::deserialize(&setup[..n]).is_err());
    }
    for n in 0..192 {
        assert!(ServerRegistration::deserialize(&record[..n]).is_err());
    }
    for n in 0..96 {
        assert!(CredentialRequest::deserialize(&ke1[..n]).is_err());
    }
    for n in 0..320 {
        assert!(CredentialResponse::deserialize(&ke2[..n]).is_err());
    }
    for n in 0..64 {
        assert!(CredentialFinalization::deserialize(&[0; 64][..n]).is_err());
    }
    for bad in [[0; 32], [255; 32]] {
        for offset in [64, 96] {
            let mut bytes = setup;
            bytes[offset..offset + 32].copy_from_slice(&bad);
            assert!(ServerSetup::deserialize(&bytes).is_err());
        }
        for offset in [0, 64] {
            let mut bytes = ke1;
            bytes[offset..offset + 32].copy_from_slice(&bad);
            assert!(CredentialRequest::deserialize(&bytes).is_err());
        }
        for offset in [0, 224] {
            let mut bytes = ke2;
            bytes[offset..offset + 32].copy_from_slice(&bad);
            assert!(CredentialResponse::deserialize(&bytes).is_err());
        }
    }
    assert_eq!(
        ServerSetup::deserialize(&[setup.as_slice(), b"extra"].concat())
            .unwrap()
            .serialize(),
        setup
    );
    assert_eq!(
        ServerRegistration::deserialize(&[record.as_slice(), b"extra"].concat())
            .unwrap()
            .serialize(),
        record
    );
}

#[test]
fn reflections_are_rejected_before_password_stretching() {
    let mut source = rng::Stream::new(12);
    let client = ClientRegistration::start(&mut source, b"password").unwrap();
    let response = RegistrationResponse {
        evaluation: client.message.0,
        server_s_pk: PublicKey(oprf::public_key(&oprf::random_scalar(&mut source))),
    };
    assert!(matches!(
        client.state.finish(
            &mut source,
            b"password",
            response,
            ClientRegistrationFinishParameters::new(Default::default(), None)
        ),
        Err(ProtocolError::ReflectedValueError)
    ));
    let client = ClientLogin::start(&mut source, b"password").unwrap();
    let mut response = CredentialResponse::deserialize(&bytes::<320>(&fixture(), "ke2")).unwrap();
    response.evaluation = client.message.blinded;
    assert!(matches!(
        client
            .state
            .finish(&mut source, b"password", response, Default::default()),
        Err(ProtocolError::ReflectedValueError)
    ));
}

#[test]
fn dummy_records_are_generated_on_both_account_paths_and_produce_full_responses() {
    for exists in [false, true] {
        let (setup, registration) = record();
        let mut source = rng::Stream::new(8);
        let request = CredentialRequest::deserialize(&bytes::<96>(&fixture(), "ke1")).unwrap();
        let result = ServerLogin::start(
            &mut source,
            &setup,
            exists.then_some(registration),
            request,
            crate::auth::CLIENT_IDENTITY,
            Default::default(),
        )
        .unwrap();
        assert_eq!(source.bytes, 160);
        assert_eq!(result.message.serialize().len(), 320);
    }
}

#[test]
fn empty_and_default_identifiers_and_context_keep_valid_registration_and_login() {
    let password = b"boundary password";
    let ksf = small_ksf();
    for defaults in [false, true] {
        let mut source = rng::Stream::new(41);
        let setup = ServerSetup::new(&mut source);
        let ids = Identifiers {
            client: (!defaults).then_some(b"".as_slice()),
            server: (!defaults).then_some(b"".as_slice()),
        };
        let client = ClientRegistration::start(&mut source, password).unwrap();
        let response = ServerRegistration::start(&setup, client.message, b"credential").unwrap();
        let registration = client
            .state
            .finish(
                &mut source,
                password,
                response.message,
                ClientRegistrationFinishParameters::new(ids, Some(&ksf)),
            )
            .unwrap();
        let client = ClientLogin::start(&mut source, password).unwrap();
        let server = ServerLogin::start(
            &mut source,
            &setup,
            Some(ServerRegistration::finish(registration.message)),
            client.message,
            b"credential",
            ServerLoginParameters {
                context: Some(b""),
                identifiers: ids,
            },
        )
        .unwrap();
        let client = client
            .state
            .finish(
                &mut source,
                password,
                server.message,
                ClientLoginFinishParameters::new(None, ids, Some(&ksf)),
            )
            .unwrap();
        assert_eq!(
            *server
                .state
                .finish(client.message, Default::default())
                .unwrap()
                .session_key,
            *client.session_key
        );
    }
}

#[test]
fn identifier_and_context_lengths_are_checked_before_encoding_can_wrap() {
    let (setup, _) = record();
    let key = Hkdf::<Sha512>::from_prk(&[0; 64]).unwrap();
    let max = vec![7; 65535];
    let too_long = vec![7; 65536];
    for input in [&max, &too_long] {
        let ids = Identifiers {
            client: Some(input),
            server: Some(input),
        };
        let result = keys::envelope_mac(&key, &[0; 32], setup.public, setup.public, ids);
        assert_eq!(result.is_ok(), input.len() == 65535);
        let request = CredentialRequest::deserialize(&bytes::<96>(&fixture(), "ke1")).unwrap();
        let response = CredentialResponse::deserialize(&bytes::<320>(&fixture(), "ke2")).unwrap();
        assert_eq!(
            keys::transcript(
                Some(input),
                Default::default(),
                setup.public,
                setup.public,
                &request,
                &response
            )
            .is_ok(),
            input.len() == 65535
        );
    }
}

#[test]
fn altered_credentials_keys_context_and_finished_macs_are_rejected() {
    let password = b"tamper fixture";
    let ksf = small_ksf();
    let mut source = rng::Stream::new(123);
    let setup = ServerSetup::new(&mut source);
    let ids = Identifiers {
        client: Some(b"client"),
        server: Some(b"server"),
    };
    let client = ClientRegistration::start(&mut source, password).unwrap();
    let response = ServerRegistration::start(&setup, client.message, b"credential").unwrap();
    let registered = client
        .state
        .finish(
            &mut source,
            password,
            response.message,
            ClientRegistrationFinishParameters::new(ids, Some(&ksf)),
        )
        .unwrap()
        .message
        .serialize();
    for case in 0..11 {
        let input = if case == 0 {
            b"wrong".as_slice()
        } else {
            password.as_slice()
        };
        let client = ClientLogin::start(&mut source, input).unwrap();
        let server = ServerLogin::start(
            &mut source,
            &setup,
            Some(ServerRegistration::deserialize(&registered).unwrap()),
            client.message,
            b"credential",
            ServerLoginParameters {
                context: Some(b"context"),
                identifiers: ids,
            },
        )
        .unwrap();
        let mut encoded = server.message.serialize();
        if case >= 4 && case < 10 {
            encoded[[32, 64, 96, 160, 192, 256][case - 4]] ^= 1;
        }
        let context = if case == 1 {
            b"wrong".as_slice()
        } else {
            b"context".as_slice()
        };
        let client_ids = Identifiers {
            client: Some(if case == 2 { b"wrong" } else { b"client" }),
            server: Some(if case == 3 { b"wrong" } else { b"server" }),
        };
        let result = CredentialResponse::deserialize(&encoded).and_then(|response| {
            client.state.finish(
                &mut source,
                input,
                response,
                ClientLoginFinishParameters::new(Some(context), client_ids, Some(&ksf)),
            )
        });
        if case < 10 {
            assert!(result.is_err(), "case {case}");
        } else {
            let mut ke3 = result.unwrap().message.serialize();
            ke3[0] ^= 1;
            assert!(
                server
                    .state
                    .finish(
                        CredentialFinalization::deserialize(&ke3).unwrap(),
                        Default::default()
                    )
                    .is_err()
            );
        }
    }
}

#[test]
fn explicit_identifiers_borrow_bytes_without_reencoding_public_points() {
    let calls = std::cell::Cell::new(0);
    let bytes = b"explicit-identity";
    let id = keys::Identifier::new(Some(bytes), || {
        calls.set(calls.get() + 1);
        [7; 32]
    });
    assert_eq!(calls.get(), 0);
    assert!(std::ptr::eq(id.as_ref().as_ptr(), bytes.as_ptr()));
    let id = keys::Identifier::new(None, || {
        calls.set(calls.get() + 1);
        [7; 32]
    });
    assert_eq!(calls.get(), 1);
    assert_eq!(id.as_ref(), [7; 32]);
}
