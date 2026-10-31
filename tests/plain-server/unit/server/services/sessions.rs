use crate::http::{auth::ws_login, graphql};
use crate::server::ContentServer;
use crate::server::ServerState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
};
use plain_rs::services::sessions::*;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;

fn fixture() -> (tempfile::TempDir, ContentServer) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("background_service_enabled", true).unwrap();
    prefs.set("desktop_access", true).unwrap();
    prefs.set("login_require_approval", false).unwrap();
    prefs.set("password", "secret").unwrap();
    prefs.set("password_type", "FIXED").unwrap();
    prefs.set("client_id", "phone").unwrap();
    let (private, _public) = plain_rs::crypto::ed25519_generate();
    prefs
        .set(
            "signature_key_pair",
            plain_rs::utils::base64::base64_encode(&private),
        )
        .unwrap();
    let server = ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    (dir, server)
}

fn offer(
    state: &ServerState,
    client: &plain_rs::crypto::EcdhSession,
    peer: Value,
) -> (
    ws_login::Request,
    plain_rs::services::confirmation_channel::Channel,
) {
    use sha2::Digest;
    let digest = plain_rs::utils::hex::bytes_to_hex(&sha2::Sha512::digest(b"secret"));
    use plain_rs::services::confirmation_channel::{Client, ConfirmationMode, ServerHandshake};
    let (login, hello) = Client::start("browser", ConfirmationMode::Password).unwrap();
    let signing = plain_rs::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
    let now = chrono::Utc::now().timestamp_millis() as u64;
    let device_id = state.native_client_id().unwrap();
    let (server, response) = ServerHandshake::start(
        "browser",
        ConfirmationMode::Password,
        &device_id,
        &hello,
        &signing,
        now,
    )
    .unwrap();
    let (keys, reveal) = login
        .accept(
            &response,
            &plain_rs::utils::base64::base64_encode(&signing[32..]),
            &device_id,
            now,
        )
        .unwrap();
    let channel = server.finish(&reveal, now).unwrap();
    let request = json!({"password":digest,"loginContext":keys.context,"browserName":"Test","browserVersion":"1","osName":"Fixture","osVersion":"1","isMobile":false,"ecdhPublicKey":plain_rs::utils::base64::base64_encode(&client.public_key_bytes),"peer":peer});
    let frame = plain_rs::crypto::xchacha_encrypt_raw(
        &digest.as_bytes()[..32],
        &serde_json::to_vec(&request).unwrap(),
    )
    .unwrap();
    let frame = keys.encrypt_request(&frame, now).unwrap();
    (
        ws_login::Request::Issue {
            client_id: "browser".into(),
            client_ip: "192.0.2.1".into(),
            frame: plain_rs::utils::base64::base64_encode(&frame),
            channel,
        },
        keys,
    )
}

#[tokio::test]
async fn web_login_ecdh_encrypted_response_signature_and_token_auth_survive_restart() {
    let (dir, server) = fixture();
    let state = server.runtime_state();
    let client = plain_rs::crypto::EcdhSession::generate();
    let (request, keys) = offer(&state, &client, Value::Null);
    let issued = ws_login::execute(&state, request).await.unwrap();
    assert_eq!(issued["status"], "COMPLETED");
    let shared = client
        .compute_shared_key(&plain_rs::utils::base64::base64_decode(
            issued["response"]["ecdhPublicKey"].as_str().unwrap(),
        ))
        .unwrap();
    assert_eq!(
        plain_rs::utils::base64::base64_encode(&shared),
        issued["token"]
    );
    let row = state.db.session_get("browser").unwrap().unwrap();
    assert_eq!(row.r#type, "WEB");
    assert_eq!(
        plain_rs::db::Db::open(&plain_rs::data_paths::DataPaths::new(dir.path()).database())
            .unwrap()
            .session_get("browser")
            .unwrap()
            .unwrap()
            .token,
        row.token
    );
    use sha2::Digest;
    let digest = plain_rs::utils::hex::bytes_to_hex(&sha2::Sha512::digest(b"secret"));
    let encrypted = keys
        .decrypt_response(
            &plain_rs::utils::base64::base64_decode(issued["frame"].as_str().unwrap()),
            chrono::Utc::now().timestamp_millis() as u64,
        )
        .unwrap();
    let decrypted =
        plain_rs::crypto::xchacha_decrypt_raw(&digest.as_bytes()[..32], &encrypted).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&decrypted).unwrap(),
        issued["response"]
    );
    let response = &issued["response"];
    let material = format!(
        "phone|COMPLETED|{}|{}|{}",
        response["ecdhPublicKey"].as_str().unwrap(),
        response["timestamp"].as_u64().unwrap(),
        keys.context,
    );
    let pair = plain_rs::prefs::security::signing_keypair(&state.prefs).unwrap();
    assert!(plain_rs::crypto::ed25519_verify(
        &plain_rs::base64_encode(&pair[32..]),
        material.as_bytes(),
        response["signature"].as_str().unwrap()
    ));
    let mut headers = HeaderMap::new();
    headers.insert("c-id", "browser".parse().unwrap());
    let plaintext = format!(
        "{}|{}|{{\"query\":\"{{noteCount(query:\\\"\\\")}}\"}}",
        chrono::Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4()
    );
    let encrypted = plain_rs::crypto::xchacha_encrypt_raw(&shared, plaintext.as_bytes()).unwrap();
    let context = plain_rs::transport::http_response::Context::new("/graphql", &encrypted);
    let response = graphql::call(
        State(state.clone()),
        headers.clone(),
        axum::body::Bytes::from(encrypted),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body = context.decrypt(&shared, &body).unwrap();
    assert!(serde_json::from_slice::<Value>(&body).unwrap()["errors"].is_null());
    headers.insert(
        "authorization",
        format!("Bearer {}", row.token).parse().unwrap(),
    );
    assert_eq!(
        graphql::call(State(state), headers, axum::body::Bytes::from("{}"))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn confirmation_is_once_only_and_cancel_or_revoke_prevents_issuance() {
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    state.prefs.set("login_require_approval", true).unwrap();
    let client = plain_rs::crypto::EcdhSession::generate();
    let pending = ws_login::execute(&state, offer(&state, &client, Value::Null).0)
        .await
        .unwrap();
    assert_eq!(pending["status"], "PENDING");
    assert!(state.db.session_get("browser").unwrap().is_none());
    let id = pending["requestId"].as_str().unwrap().to_owned();
    ws_login::execute(
        &state,
        ws_login::Request::Complete {
            request_id: id.clone(),
        },
    )
    .await
    .unwrap();
    assert!(
        ws_login::execute(&state, ws_login::Request::Complete { request_id: id })
            .await
            .is_err()
    );
    for cancelled in [true, false] {
        let pending = ws_login::execute(&state, offer(&state, &client, Value::Null).0)
            .await
            .unwrap();
        let id = pending["requestId"].as_str().unwrap().to_owned();
        if cancelled {
            ws_login::execute(
                &state,
                ws_login::Request::Cancel {
                    request_id: id.clone(),
                },
            )
            .await
            .unwrap();
        } else {
            delete(&state, "browser").unwrap();
        }
        assert!(
            ws_login::execute(&state, ws_login::Request::Complete { request_id: id })
                .await
                .is_err()
        );
    }
}
#[cfg(feature = "http_transport")]
#[tokio::test]
async fn native_management_public_auth_and_revoke_share_one_database_without_host() {
    let (dir, server) = fixture();
    let state = server.runtime_state();
    let created = serde_json::to_value(create(&state, "API".into()).unwrap()).unwrap();
    let id = created["client_id"].as_str().unwrap();
    let token = created["token"].as_str().unwrap();
    assert_eq!(plain_rs::utils::base64::base64_decode(token).len(), 32);
    let mut headers = HeaderMap::new();
    headers.insert("c-id", id.parse().unwrap());
    headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
    let response = graphql::call(
        State(state.clone()),
        headers.clone(),
        axum::body::Bytes::from(r#"{"query":"{noteCount(query:\"\")}"}"#),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        state
            .db
            .session_get(id)
            .unwrap()
            .unwrap()
            .last_seen_at
            .is_some()
    );
    assert!(rename(&state, id, "Renamed").unwrap());
    let reopened =
        plain_rs::db::Db::open(&plain_rs::data_paths::DataPaths::new(dir.path()).database())
            .unwrap();
    assert_eq!(reopened.session_get(id).unwrap().unwrap().name, "Renamed");
    let mut wrong = headers.clone();
    wrong.insert("authorization", "Bearer wrong".parse().unwrap());
    assert_eq!(
        graphql::call(State(state.clone()), wrong, axum::body::Bytes::from("{}"))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(delete(&state, id).unwrap());
    assert!(key(&state.db, id).unwrap().is_none());
    assert_eq!(
        graphql::call(State(state.clone()), headers, axum::body::Bytes::from("{}"))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(state.db.session_list().unwrap().is_empty());
    let unauthorized = state.public.execute("{ sessions { clientId } }").await;
    assert!(!unauthorized.errors.is_empty());
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn login_pairing_and_init_preserve_rust_state_without_revealing_passwords() {
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    let client = plain_rs::crypto::EcdhSession::generate();
    let (_, public) = plain_rs::crypto::ed25519_generate();
    let peer = json!({"port":8443,"signaturePublicKey":plain_rs::utils::base64::base64_encode(&public),"deviceName":"Desktop","deviceType":"COMPUTER","ips":["192.0.2.1"]});
    let pending = ws_login::execute(&state, offer(&state, &client, peer).0)
        .await
        .unwrap();
    assert_eq!(
        pending["status"], "PENDING",
        "pairing requires confirmation even with browser 2FA off"
    );
    let issued = ws_login::execute(
        &state,
        ws_login::Request::Complete {
            request_id: pending["requestId"].as_str().unwrap().into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(issued["response"]["chatPaired"], true);
    let paired = state.db.get_peer_by_id("browser").unwrap();
    use sha2::Digest;
    let expected = sha2::Sha256::digest(
        [
            b"plain-chat-pairing-v1".as_slice(),
            plain_rs::utils::base64::base64_decode(issued["token"].as_str().unwrap()).as_slice(),
        ]
        .concat(),
    );
    assert_eq!(
        paired.key,
        plain_rs::utils::base64::base64_encode(&expected)
    );
    assert_eq!(paired.name, "Desktop");
    let token = key(&state.db, "browser").unwrap().unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("c-id", "browser".parse().unwrap());
    let response = crate::http::init::init(
        State(state.clone()),
        axum::extract::ConnectInfo("127.0.0.1:1".parse().unwrap()),
        headers.clone(),
        axum::body::Bytes::from(plain_rs::crypto::xchacha_encrypt_raw(&token, b"init").unwrap()),
    )
    .await;
    let response: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(response.get("password").is_none());
    assert_eq!(response["authenticated"], true);
    assert_eq!(response["passwordRequired"], true);
    assert!(!response["signaturePublicKey"].as_str().unwrap().is_empty());
    state.prefs.set("password_type", "NONE").unwrap();
    let response = crate::http::init::init(
        State(state.clone()),
        axum::extract::ConnectInfo("127.0.0.1:1".parse().unwrap()),
        headers,
        axum::body::Bytes::new(),
    )
    .await;
    let response: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(response.get("password").is_none());
    assert_eq!(response["authenticated"], false);
    assert_eq!(response["passwordRequired"], false);
    assert_eq!(state.prefs.get_or("password", String::new()), "secret");
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn main_schema_keeps_secrets_local_and_remote_metadata_and_resource_token_available() {
    use plain_rs::db::chat_store::{SaveMode, channels, peers};
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    let session = create(&state, "Remote".into()).unwrap();
    state.prefs.set("url_token", "resource-token").unwrap();
    state.prefs.set("adb_token", "adb-secret").unwrap();
    state.prefs.set("dev_token", "dev-secret").unwrap();
    let mut peer = plain_rs::db::DPeer::new(
        "peer",
        "Peer",
        "192.0.2.2",
        Some(8080),
        plain_rs::chat::enums::DeviceType::Tablet,
    );
    peer.key = "peer-secret".into();
    peer.token = "peer-token".into();
    peers::save(&state.db, &[peer], SaveMode::Insert).unwrap();
    let mut channel = plain_rs::db::DChannel::new("Channel", "peer");
    channel.id = "channel".into();
    channel.key = "channel-secret".into();
    channels::save(&state.db, &[channel], SaveMode::Insert).unwrap();
    let native = server.native_graphql().unwrap();
    let query = r#"{ sessions { clientId token } peers { key token } chatChannels { key } }"#;
    let local = native.execute(query.into()).await;
    assert!(local.errors.is_empty(), "{:?}", local.errors);
    let local = local.data.into_json().unwrap();
    let settings = server.native_management().unwrap().preferences().unwrap();
    assert_eq!(settings["password"], "secret");
    assert_eq!(settings["url_token"], "resource-token");
    assert_eq!(local["peers"][0]["key"], "peer-secret");
    assert_eq!(local["chatChannels"][0]["key"], "channel-secret");
    assert_eq!(local["sessions"][0]["token"], session.token);
    let mut headers = HeaderMap::new();
    headers.insert("c-id", session.client_id.parse().unwrap());
    headers.insert(
        "authorization",
        format!("Bearer {}", session.token).parse().unwrap(),
    );
    async fn remote(state: &ServerState, headers: &HeaderMap, query: &str) -> Value {
        let response = graphql::call(
            State(state.clone()),
            headers.clone(),
            axum::body::Bytes::from(serde_json::to_vec(&json!({"query":query})).unwrap()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap()
    }
    let metadata = remote(&state, &headers,
        r#"{ sessions { clientId clientName } peers { id name } chatChannels { id name } app { urlToken } }"#).await;
    assert!(metadata.get("errors").is_none(), "{metadata}");
    assert_eq!(metadata["data"]["app"]["urlToken"], session.url_token);
    assert_eq!(
        metadata["data"]["sessions"][0]["clientId"],
        session.client_id
    );
    assert_eq!(metadata["data"]["peers"][0]["id"], "peer");
    for query in [
        "{sessions{token}}",
        "{peers{key}}",
        "{peers{token}}",
        "{peers{key}}",
        "{chatChannels{key}}",
    ] {
        let denied = remote(&state, &headers, query).await;
        assert_eq!(
            denied["errors"][0]["message"], "no_permission",
            "{query}: {denied}"
        );
    }
    assert_eq!(
        state
            .db
            .session_get(&session.client_id)
            .unwrap()
            .unwrap()
            .token,
        session.token
    );
    server.shutdown().await;
}
