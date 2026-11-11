use super::*;

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn invalid_password_proof_is_recorded_once_without_session_or_secrets() {
    use futures_util::SinkExt;
    use opaque_ke::{ClientLogin, ClientLoginFinishParameters, CredentialResponse, Identifiers};
    use plain_libs::tokio_tungstenite::tungstenite::Message;
    let (_directory, server, port) = passwordless_server().await;
    let state = server.runtime_state();
    plain_rs::features::prefs::security::set_password_login(&state.prefs, "fixture-password").unwrap();
    let signing = plain_rs::features::prefs::security::signing_keypair(&state.prefs).unwrap();
    let identity = &signing[32..];
    let context = plain_libs::auth::context("bad-proof", identity);
    let client = ClientLogin::<plain_libs::auth::Suite>::start(&mut rand_core::OsRng, b"fixture-password").unwrap();
    let (mut socket, _) = plain_libs::tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/auth?cid=bad-proof")).await.unwrap();
    socket.send(Message::Binary(client.message.serialize().to_vec().into())).await.unwrap();
    let Message::Binary(response) = login_frame(&mut socket).await else { panic!("OPAQUE response required") };
    let ksf = plain_libs::auth::ksf();
    let finish = client.state.finish(&mut rand_core::OsRng, b"fixture-password", CredentialResponse::deserialize(&response).unwrap(), ClientLoginFinishParameters {
        context: Some(&context), identifiers: Identifiers { client: Some(plain_libs::auth::CLIENT_IDENTITY), server: Some(identity) }, ksf: Some(&ksf),
    }).unwrap();
    let mut proof = finish.message.serialize().to_vec();
    proof[0] ^= 1;
    socket.send(Message::Binary(proof.into())).await.unwrap();
    assert!(matches!(login_frame(&mut socket).await, Message::Close(Some(frame)) if frame.reason == "invalid_password"));
    let records = state.db.audit_events(0, 10, None).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].r#type, "login_failed");
    assert_eq!(records[0].message, "invalid_password");
    assert_eq!(records[0].client_id.as_deref(), Some("bad-proof"));
    assert!(!records[0].message.contains("fixture-password"));
    assert!(state.db.session_get("bad-proof").unwrap().is_none());
    let response = server.graphql_executor().execute(plain_graphql::Request::new("{events(query:\"\",offset:0,limit:10){type message clientId}}")).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["events"][0]["type"], "LOGIN_FAILED");
    let old = server.graphql_executor().execute(plain_graphql::Request::new("{auditEvents(query:\"\",offset:0,limit:10){id}}")).await;
    assert!(!old.errors.is_empty());
    server.shutdown().await;
}
