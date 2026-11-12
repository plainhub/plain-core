use super::*;
use plain_rs::infrastructure::db::SessionRow;
use plain_rs::infrastructure::transport::http_request;

fn headers(session: &SessionRow) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("c-id", session.client_id.parse().unwrap());
    headers
}

fn seal(session: &SessionRow, plain: &str) -> Bytes {
    let key = plain_rs::features::sessions::token_key(&session.token).unwrap();
    http_request::encrypt_raw(&key, "POST", "/graphql", plain.as_bytes())
        .unwrap()
        .into()
}

fn request(id: &str, query: &str) -> String {
    plain_libs::serde_json::json!({"requestId":id,"query":query}).to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_replay_executes_the_mutation_once() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::open(dir.path()).unwrap();
    let state = server.runtime_state();
    let session = plain_rs::features::services::sessions::create(&state, "fixture".into()).unwrap();
    let body = seal(
        &session,
        &request(
            &plain_rs::features::record_id::generate(),
            "mutation { createNote(input:{ title:\"once\",content:\"\" }) { id } }",
        ),
    );
    let results = futures_util::future::join_all(
        (0..16).map(|_| tokio::spawn(call(State(state.clone()), headers(&session), body.clone()))),
    )
    .await;
    assert_eq!(
        results
            .iter()
            .filter(|r| r.as_ref().unwrap().status() == StatusCode::OK)
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|r| r.as_ref().unwrap().status() == StatusCode::BAD_REQUEST)
            .count(),
        15
    );
    let result = run_with_context(
        &state.public,
        &plain_libs::serde_json::json!({"query":r#"{ noteCount(query:"") }"#}).to_string(),
        crate::graphql::executor::RequestContext::authenticated_remote(
            state.clone(),
            session.client_id.clone(),
        ),
    )
    .await
    .unwrap();
    let value: plain_libs::serde_json::Value = plain_libs::serde_json::from_slice(&result).unwrap();
    assert_eq!(value["data"]["noteCount"], 1);
    server.stop_core().await;
}

#[tokio::test]
async fn malformed_and_unauthed_ids_never_consume_a_valid_request() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::open(dir.path()).unwrap();
    let state = server.runtime_state();
    let session = plain_rs::features::services::sessions::create(&state, "fixture".into()).unwrap();
    let query = "{ __typename }";
    for plain in [
        r#"{"query":"{ __typename }"}"#.to_owned(),
        request(&plain_libs::utils::uuid::Uuid::new_v4().to_string(), query),
        request("not-a-uuid", query),
        format!(
            "{}|{}|{{}}",
            plain_libs::chrono::Utc::now().timestamp_millis(),
            plain_libs::utils::uuid::Uuid::new_v4()
        ),
    ] {
        assert_eq!(
            call(
                State(state.clone()),
                headers(&session),
                seal(&session, &plain)
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }
    let plain = request(&plain_rs::features::record_id::generate(), query);
    let key = plain_rs::features::sessions::token_key(&session.token).unwrap();
    let wrong_route =
        http_request::encrypt_raw(&key, "POST", "/guest_graphql", format!("{}|{}|{{}}", plain_libs::chrono::Utc::now().timestamp_millis(), plain_libs::utils::uuid::Uuid::new_v4()).as_bytes()).unwrap();
    assert_eq!(
        call(State(state.clone()), headers(&session), wrong_route.into())
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let original = seal(&session, &plain);
    let mut tampered = original.to_vec();
    *tampered.last_mut().unwrap() ^= 1;
    assert_eq!(
        call(State(state.clone()), headers(&session), tampered.into())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(State(state.clone()), headers(&session), original.clone())
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        call(State(state.clone()), headers(&session), original)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    server.stop_core().await;
}

#[tokio::test]
async fn restart_rejects_the_old_key_without_deleting_sessions_and_rekey_recovers() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::open(dir.path()).unwrap();
    let state = server.runtime_state();
    let session = plain_rs::features::services::sessions::create(&state, "fixture".into()).unwrap();
    let plain = request(&plain_rs::features::record_id::generate(), "{ __typename }");
    let body = seal(&session, &plain);
    assert_eq!(
        call(State(state.clone()), headers(&session), body.clone())
            .await
            .status(),
        StatusCode::OK
    );
    server.stop_core().await;
    drop(state);
    drop(server);
    let server = crate::server::ContentServer::open(dir.path()).unwrap();
    let state = server.runtime_state();
    assert_eq!(
        state
            .db
            .session_get(&session.client_id)
            .unwrap()
            .unwrap()
            .token,
        session.token
    );
    assert_eq!(
        call(State(state.clone()), headers(&session), body.clone())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let mut bearer = headers(&session);
    bearer.insert(
        "authorization",
        format!("Bearer {}", session.token).parse().unwrap(),
    );
    assert_eq!(
        call(
            State(state.clone()),
            bearer,
            Bytes::from_static(br#"{"query":"{ __typename }"}"#)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let mut renewed = session.clone();
    renewed.token = plain_libs::utils::base64::base64_encode(&plain_libs::crypto::random_bytes(32));
    state.db.session_save(&renewed).unwrap();
    assert_eq!(
        call(State(state.clone()), headers(&session), body)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            State(state.clone()),
            headers(&renewed),
            seal(&renewed, &plain)
        )
        .await
        .status(),
        StatusCode::OK
    );
    server.stop_core().await;
}

#[tokio::test]
async fn fresh_sessions_accept_the_same_uuid_independently() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::open(dir.path()).unwrap();
    let state = server.runtime_state();
    let plain = request(&plain_rs::features::record_id::generate(), "{ __typename }");
    for _ in 0..2 {
        let session =
            plain_rs::features::services::sessions::create(&state, "fixture".into()).unwrap();
        assert_eq!(
            call(
                State(state.clone()),
                headers(&session),
                seal(&session, &plain)
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    server.stop_core().await;
}

#[tokio::test]
async fn distinct_keys_reject_cross_route_frames_and_signed_envelopes_require_peer_syntax() {
    use plain_rs::features::chat::enums::{DeviceType, PeerStatus};
    use plain_rs::features::files::shares::Service;
    use plain_rs::features::prefs::Prefs;
    use plain_rs::infrastructure::db::DPeer;
    use plain_rs::infrastructure::transport::{contract, http_response};
    use plain_libs::utils::base64::{base64_decode_checked, base64_encode};
    use std::sync::Arc;

    let dir = plain_libs::temporary::tempdir().unwrap();
    let folder = dir.path().join("shared");
    std::fs::create_dir(&folder).unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("master_secret", base64_encode(&[11; 32])).unwrap();
    let server = crate::server::ContentServer::start(dir.path(), &base64_encode(&[10; 32]), prefs).unwrap();
    let state = server.runtime_state();
    let service = Service::new(state.db.clone(), state.prefs.clone());
    let share = service.create("fixture".into(), vec![folder.to_str().unwrap().into()], true, None).unwrap();
    let key = base64_decode_checked(&service.token(&share.id).unwrap()).unwrap();
    let session = plain_rs::features::services::sessions::create(&state, "fixture".into()).unwrap();
    let main_key = plain_rs::features::sessions::token_key(&session.token).unwrap();
    let peer_key = vec![12; 32];
    assert_ne!(main_key.as_slice(), key.as_slice());
    assert_ne!(peer_key, key);
    let (keypair, public_key) = plain_libs::crypto::ed25519_generate();
    let mut peer = DPeer::new("fixture-peer", "fixture", "", Some(443), DeviceType::Phone);
    peer.status = PeerStatus::Paired;
    peer.key = base64_encode(&peer_key);
    peer.public_key = base64_encode(&public_key);
    state.db.upsert_peer(&peer);
    let document = r#"{"query":"{ __typename }"}"#;
    let payloads = [
        request(&plain_rs::features::record_id::generate(), "{ __typename }"),
        request(&plain_rs::features::record_id::generate(), "{ __typename }"),
        contract::signed_request(&keypair, "{ __typename }", plain_libs::serde_json::json!({}), "").unwrap(),
    ];
    let keys: [&[u8]; 3] = [&main_key, &key, &peer_key];
    let routes = ["/graphql", "/guest_graphql", "/peer_graphql"];
    let frames: Vec<Vec<u8>> = payloads.iter().enumerate().map(|(index, plain)| plain_libs::crypto::xchacha_encrypt_raw(keys[index], plain.as_bytes()).unwrap()).collect();
    for (source, frame) in frames.iter().enumerate() {
        for (target, route) in routes.iter().enumerate() {
            assert_eq!(http_request::decrypt_raw(keys[source], "POST", route, frame).unwrap(), payloads[source].as_bytes());
            if source == target { continue; }
            let status = match target {
                0 => call(State(state.clone()), headers(&session), frame.clone().into()).await.status().as_u16(),
                1 => execute_guest(&state, &share.id, frame).await.0,
                _ => crate::graphql::schemas::peer::execute(&state, &peer.id, "", frame).await.0,
            };
            assert_eq!(status, 401, "{} -> {route}: {status}", routes[source]);
        }
        let old = plain_libs::crypto::xchacha_encrypt_raw(keys[source], &[&[(source + 1) as u8][..], payloads[source].as_bytes()].concat()).unwrap();
        let status = match source {
            0 => call(State(state.clone()), headers(&session), old.into()).await.status().as_u16(),
            1 => execute_guest(&state, &share.id, &old).await.0,
            _ => crate::graphql::schemas::peer::execute(&state, &peer.id, "", &old).await.0,
        };
        assert!(matches!(status, 400 | 401));
    }
    // Re-encrypt with the destination key to isolate syntax from key-domain rejection.
    for target in 0..3 {
        let foreign = if target == 2 { &payloads[0] } else { &payloads[2] };
        let frame = plain_libs::crypto::xchacha_encrypt_raw(keys[target], foreign.as_bytes()).unwrap();
        let status = match target {
            0 => call(State(state.clone()), headers(&session), frame.into()).await.status().as_u16(),
            1 => execute_guest(&state, &share.id, &frame).await.0,
            _ => crate::graphql::schemas::peer::execute(&state, &peer.id, "", &frame).await.0,
        };
        assert!(matches!(status, 400 | 401));
        let old_plain = format!("signature|{}|nonce|{document}", plain_libs::chrono::Utc::now().timestamp_millis());
        let old = plain_libs::crypto::xchacha_encrypt_raw(keys[target], old_plain.as_bytes()).unwrap();
        let status = match target {
            0 => call(State(state.clone()), headers(&session), old.into()).await.status().as_u16(),
            1 => execute_guest(&state, &share.id, &old).await.0,
            _ => crate::graphql::schemas::peer::execute(&state, &peer.id, "", &old).await.0,
        };
        assert!(matches!(status, 400 | 401));
    }
    assert_eq!(call(State(state.clone()), headers(&session), frames[0].clone().into()).await.status(), StatusCode::OK);
    for (index, (status, response)) in [execute_guest(&state, &share.id, &frames[1]).await, crate::graphql::schemas::peer::execute(&state, &peer.id, "", &frames[2]).await].into_iter().enumerate() {
        assert_eq!(status, 200);
        assert!(http_response::Context::new(&frames[index + 1]).decrypt(keys[index + 1], &response).is_some());
        assert!(http_response::Context::new(&frames[0]).decrypt(keys[index + 1], &response).is_none());
    }
    assert_eq!(call(State(state.clone()), headers(&session), frames[0].clone().into()).await.status(), StatusCode::BAD_REQUEST);
    assert_eq!(execute_guest(&state, &share.id, &frames[1]).await.0, 400);
    assert_eq!(crate::graphql::schemas::peer::execute(&state, &peer.id, "", &frames[2]).await.0, 400);
    server.shutdown().await;
}
