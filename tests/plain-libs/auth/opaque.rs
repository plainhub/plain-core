use plain_libs::auth::{server, context, ksf, CLIENT_IDENTITY};
use plain_libs::opaque::{ClientLogin, ClientLoginFinishParameters, CredentialResponse, Identifiers};
use rand_core::OsRng;

#[test]
fn registration_and_login_bind_identity_context_and_key() {
    let identity = [7; 32];
    let registration = server::register(b"correct horse", &identity).unwrap();
    let context = context("client", &identity);
    let client = ClientLogin::start(&mut OsRng, b"correct horse").unwrap();
    let (server, ke2) = server::start(&registration.setup, &registration.record,
        &client.message.serialize(), &context, &identity).unwrap();
    let ksf = ksf();
    let client = client.state.finish(&mut OsRng, b"correct horse",
        CredentialResponse::deserialize(&ke2).unwrap(), ClientLoginFinishParameters {
            context: Some(&context), identifiers: Identifiers { client: Some(CLIENT_IDENTITY), server: Some(&identity) }, ksf: Some(&ksf),
        }).unwrap();
    let key = server.finish(&client.message.serialize()).unwrap();
    assert_eq!(key.as_slice(), client.session_key.as_slice());
    assert_ne!(plain_libs::auth::api_key(&key, &context), plain_libs::auth::api_key(&key, b"other"));
    assert_ne!(plain_libs::auth::auth_aad(&context, false), plain_libs::auth::auth_aad(&context, true));
}

#[test]
fn rejects_wrong_password_context_identity_and_tampered_messages() {
    let identity = [7; 32];
    let registration = server::register(b"correct horse", &identity).unwrap();
    let context = context("client", &identity);
    for case in 0..5 {
        let password = if case == 0 { b"wrong password".as_slice() } else { b"correct horse".as_slice() };
        let client = ClientLogin::start(&mut OsRng, password).unwrap();
        let (server, mut ke2) = server::start(&registration.setup, &registration.record,
            &client.message.serialize(), &context, &identity).unwrap();
        if case == 3 { let last = ke2.len()-1; ke2[last] ^= 1; }
        let ksf = ksf();
        let result = CredentialResponse::deserialize(&ke2).and_then(|response|
            client.state.finish(&mut OsRng, password, response, ClientLoginFinishParameters {
                context: Some(if case == 1 { b"wrong context" } else { &context }),
                identifiers: Identifiers { client: Some(CLIENT_IDENTITY), server: Some(if case == 2 { &[8;32] } else { &identity }) },
                ksf: Some(&ksf),
            }));
        if case < 4 { assert!(result.is_err(), "case {case}"); }
        else {
            let mut ke3 = result.unwrap().message.serialize().to_vec();
            assert_eq!(ke3.len(), server::finalization_len());
            ke3[0] ^= 1;
            assert!(server.finish(&ke3).is_err());
        }
    }
}

#[test]
fn independent_logins_have_different_keys() {
    let identity = [7;32];
    let registration = server::register(b"correct horse", &identity).unwrap();
    let context = context("client", &identity);
    let mut keys = Vec::new();
    for _ in 0..2 {
        let client = ClientLogin::start(&mut OsRng, b"correct horse").unwrap();
        let (server, ke2) = server::start(&registration.setup,&registration.record,&client.message.serialize(),&context,&identity).unwrap();
        let ksf = ksf();
        let result = client.state.finish(&mut OsRng,b"correct horse",CredentialResponse::deserialize(&ke2).unwrap(),ClientLoginFinishParameters {
            context:Some(&context),identifiers:Identifiers{client:Some(CLIENT_IDENTITY),server:Some(&identity)},ksf:Some(&ksf),
        }).unwrap();
        keys.push(server.finish(&result.message.serialize()).unwrap());
    }
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn native_client_wrapper_verifies_password_pin_and_derived_outputs() {
    let identity = [7; 32];
    let registration = server::register(b"native test password", &identity).unwrap();
    let public_key = server::public_key(&registration.setup).unwrap();
    let context = context("native-client", &identity);
    for case in 0..3 {
        let password = if case == 1 { b"wrong".to_vec() } else { b"native test password".to_vec() };
        let client = plain_libs::auth::client::OpaqueLogin::start(password).unwrap();
        let (server, ke2) = server::start(&registration.setup, &registration.record,
            &client.request(), &context, &identity).unwrap();
        let result = client.finish(&ke2, "native-client", &identity,
            if case == 2 { &[0; 32] } else { &public_key });
        if case != 0 {
            assert!(result.is_err(), "case {case}");
            continue;
        }
        let result = result.unwrap();
        let session = server.finish(&result.ke3).unwrap();
        assert_eq!(result.api_key.as_slice(), plain_libs::auth::api_key(&session, &context));
        assert_eq!(result.request_aad, plain_libs::auth::auth_aad(&context, false));
        assert_eq!(result.response_aad, plain_libs::auth::auth_aad(&context, true));
        assert_eq!(result.server_public_key, public_key);
    }
}

#[test]
fn context_binds_fixed_login_purpose_client_and_server_identity() {
    let identity = [7; 32];
    let mut expected = vec![0, 0, 0, 14];
    expected.extend_from_slice(b"LOGIN_PASSWORD");
    expected.extend_from_slice(&[0, 0, 0, 6]);
    expected.extend_from_slice(b"client");
    expected.extend_from_slice(&[0, 0, 0, 32]);
    expected.extend_from_slice(&identity);
    assert_eq!(context("client", &identity), expected);
}
