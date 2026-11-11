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
        http_request::encrypt_raw(&key, "POST", "/guest_graphql", plain.as_bytes()).unwrap();
    assert_eq!(
        call(State(state.clone()), headers(&session), wrong_route.into())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
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
