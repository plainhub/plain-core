use super::*;
use futures_util::SinkExt;
use plain_libs::serde_json::json;
use plain_libs::tokio_tungstenite::tungstenite::Message as ClientMessage;

#[derive(Clone, Copy)]
enum InvalidFrame {
    PlainInit,
    WrongKey,
    PlainMessage,
    CorruptTag,
    Replay,
}

#[tokio::test]
async fn no_subprotocol_keeps_encrypted_handshake_and_frame_validation() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let directory = plain_libs::temporary::tempdir().unwrap();
        let server = crate::server::ContentServer::open(directory.path()).unwrap();
        let state = server.runtime_state();
        let session = plain_rs::features::services::sessions::create(&state, "fixture".into()).unwrap();
        let client = session.client_id;
        let key = plain_libs::utils::base64::base64_decode(&session.token);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = plain_libs::axum::Router::new()
            .route("/graphql", plain_libs::axum::routing::get(upgrade))
            .with_state(state);
        let serving = tokio::spawn(async { plain_libs::axum::serve(listener, app).await.unwrap() });
        for invalid in [
            InvalidFrame::PlainInit,
            InvalidFrame::WrongKey,
            InvalidFrame::PlainMessage,
            InvalidFrame::CorruptTag,
            InvalidFrame::Replay,
        ] {
            let (mut socket, response) = plain_libs::tokio_tungstenite::connect_async(
                format!("ws://{address}/graphql?cid={client}"),
            )
            .await
            .expect("WebSocket upgrade must not require a subprotocol");
            assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
            assert!(response.headers().get("sec-websocket-protocol").is_none());
            let init_json = json!({
                "clientNonce": plain_libs::utils::hex::bytes_to_hex(&plain_libs::crypto::random_bytes(32)),
            });
            let init = graphql::seal_init(&key, &client, "/graphql", init_json.to_string().as_bytes()).unwrap();
            let first = match invalid {
                InvalidFrame::PlainInit => ClientMessage::Text(init_json.to_string().into()),
                InvalidFrame::WrongKey => ClientMessage::Binary(
                    graphql::seal_init(&[0; 32], &client, "/graphql", init_json.to_string().as_bytes()).unwrap().into(),
                ),
                _ => ClientMessage::Binary(init.clone().into()),
            };
            socket.send(first).await.unwrap();
            if !matches!(invalid, InvalidFrame::PlainInit | InvalidFrame::WrongKey) {
                let ClientMessage::Binary(ack) = socket.next().await.unwrap().unwrap() else {
                    panic!("encrypted acknowledgement required")
                };
                let ack = ack.to_vec();
                let plain = graphql::open_ack(&key, &client, "/graphql", &init, &ack).unwrap();
                let ack_json: graphql::Ack = plain_libs::serde_json::from_slice(&plain).unwrap();
                assert_eq!(ack_json.server_nonce.len(), 64);
                let mut stream = graphql::Stream::new("/graphql", &client, &init, &ack, Role::Client).unwrap();
                let ping = br#"{"type":"ping","payload":{"encrypted":true}}"#;
                let original = stream.encode(ping, &key).unwrap();
                socket.send(ClientMessage::Binary(original.clone().into())).await.unwrap();
                let ClientMessage::Binary(pong) = socket.next().await.unwrap().unwrap() else {
                    panic!("encrypted pong required")
                };
                let decoded = stream.decode(&pong, &key).unwrap();
                let message: plain_libs::serde_json::Value = plain_libs::serde_json::from_slice(&decoded).unwrap();
                assert_eq!(message, json!({"type":"pong","payload":{"encrypted":true}}));
                let frame = match invalid {
                    InvalidFrame::PlainMessage => ClientMessage::Text(String::from_utf8(ping.to_vec()).unwrap().into()),
                    InvalidFrame::CorruptTag => {
                        let mut encrypted = stream.encode(ping, &key).unwrap();
                        *encrypted.last_mut().unwrap() ^= 1;
                        ClientMessage::Binary(encrypted.into())
                    }
                    InvalidFrame::Replay => ClientMessage::Binary(original.into()),
                    _ => unreachable!(),
                };
                socket.send(frame).await.unwrap();
            }
            let ClientMessage::Close(Some(close)) = socket.next().await.unwrap().unwrap() else {
                panic!("invalid encrypted frame must close the connection")
            };
            assert_eq!(u16::from(close.code), 1008);
            assert_eq!(close.reason, "invalid_subscription");
        }
        serving.abort();
        server.shutdown().await;
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn reconnect_reuses_no_replay_cache_and_requires_proof_of_the_new_ack() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let directory = plain_libs::temporary::tempdir().unwrap();
        let server = crate::server::ContentServer::open(directory.path()).unwrap();
        let state = server.runtime_state();
        let session =
            plain_rs::features::services::sessions::create(&state, "fixture".into()).unwrap();
        let client = session.client_id;
        let key = plain_libs::utils::base64::base64_decode(&session.token);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = plain_libs::axum::Router::new()
            .route("/graphql", plain_libs::axum::routing::get(upgrade))
            .with_state(state.clone());
        let serving = tokio::spawn(async { plain_libs::axum::serve(listener, app).await.unwrap() });
        let init = graphql::seal_init(
            &key,
            &client,
            "/graphql",
            json!({"clientNonce":"ab".repeat(32)})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        let mut previous_ack = Vec::new();
        let mut old_frame = Vec::new();
        for attempt in 0..3 {
            let (mut socket, _) = plain_libs::tokio_tungstenite::connect_async(format!(
                "ws://{address}/graphql?cid={client}"
            ))
            .await
            .unwrap();
            socket
                .send(ClientMessage::Binary(init.clone().into()))
                .await
                .unwrap();
            let ClientMessage::Binary(ack) = socket.next().await.unwrap().unwrap() else {
                panic!("expected fresh acknowledgement")
            };
            assert_ne!(ack.as_ref(), previous_ack.as_slice());
            assert!(
                state.subscription_sessions.snapshot().is_empty(),
                "init alone must not mark the client online"
            );
            let mut stream =
                graphql::Stream::new("/graphql", &client, &init, &ack, Role::Client).unwrap();
            let ping = stream.encode(br#"{"type":"ping"}"#, &key).unwrap();
            if attempt == 1 {
                socket
                    .send(ClientMessage::Binary(old_frame.clone().into()))
                    .await
                    .unwrap();
            } else {
                socket
                    .send(ClientMessage::Binary(ping.clone().into()))
                    .await
                    .unwrap();
                let ClientMessage::Binary(pong) = socket.next().await.unwrap().unwrap() else {
                    panic!("expected authenticated pong")
                };
                assert_eq!(
                    plain_libs::serde_json::from_slice::<plain_libs::serde_json::Value>(
                        &stream.decode(&pong, &key).unwrap()
                    )
                    .unwrap()["type"],
                    "pong"
                );
                assert_eq!(state.subscription_sessions.snapshot(), vec![client.clone()]);
                socket
                    .send(ClientMessage::Binary(ping.clone().into()))
                    .await
                    .unwrap();
                old_frame = ping;
            }
            let ClientMessage::Close(Some(frame)) = socket.next().await.unwrap().unwrap() else {
                panic!("replay must close the connection")
            };
            assert_eq!(u16::from(frame.code), 1008);
            assert!(state.subscription_sessions.snapshot().is_empty());
            previous_ack = ack.to_vec();
        }
        serving.abort();
        server.shutdown().await;
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn pending_confirmation_cannot_consume_authenticated_session_capacity() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let directory = plain_libs::temporary::tempdir().unwrap();
        let server = crate::server::ContentServer::open(directory.path()).unwrap();
        let state = server.runtime_state();
        state
            .prefs
            .set(
                plain_rs::features::prefs::pref_keys::CLIENT_ID,
                "fixture-native",
            )
            .unwrap();
        let native = server.native_graphql().unwrap();
        for _ in 0..127 {
            native
                .open_subscriptions(NativeRequestScope::default())
                .unwrap();
        }
        let session =
            plain_rs::features::services::sessions::create(&state, "fixture".into()).unwrap();
        let client = session.client_id;
        let key = plain_libs::utils::base64::base64_decode(&session.token);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = plain_libs::axum::Router::new()
            .route("/graphql", plain_libs::axum::routing::get(upgrade))
            .with_state(state.clone());
        let serving = tokio::spawn(async { plain_libs::axum::serve(listener, app).await.unwrap() });
        let (mut socket, _) = plain_libs::tokio_tungstenite::connect_async(format!(
            "ws://{address}/graphql?cid={client}"
        ))
        .await
        .unwrap();
        let init = graphql::seal_init(
            &key,
            &client,
            "/graphql",
            json!({"clientNonce":"ab".repeat(32)})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        socket
            .send(ClientMessage::Binary(init.into()))
            .await
            .unwrap();
        assert!(matches!(
            socket.next().await,
            Some(Ok(ClientMessage::Binary(_)))
        ));
        let last = native
            .open_subscriptions(NativeRequestScope::default())
            .expect("unconfirmed init must not occupy the final authenticated slot");
        assert!(
            native
                .open_subscriptions(NativeRequestScope::default())
                .is_err(),
            "the authenticated capacity limit must remain enforced"
        );
        native.close_subscriptions(last);
        socket.close(None).await.unwrap();
        serving.abort();
        server.shutdown().await;
    })
    .await
    .unwrap();
}
