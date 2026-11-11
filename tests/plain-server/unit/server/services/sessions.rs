use crate::http::{auth::ws_login, graphql};
use crate::server::ContentServer;
use crate::server::ServerState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
};
use plain_rs::features::services::sessions::*;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;

fn fixture() -> (plain_libs::temporary::TempDir, ContentServer) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("background_service_enabled", true).unwrap();
    prefs.set("remote_access", true).unwrap();
    prefs.set("login_require_approval", false).unwrap();
    plain_rs::features::prefs::security::set_entry(&prefs, "password", serde_json::json!("secret"))
        .unwrap();
    prefs.set("password_type", "NONE").unwrap();
    prefs.set("client_id", "phone").unwrap();
    let (private, _public) = plain_libs::crypto::ed25519_generate();
    prefs
        .set(
            "signature_key_pair",
            plain_libs::utils::base64::base64_encode(&private),
        )
        .unwrap();
    let server = ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    (dir, server)
}

fn offer(
    state: &ServerState,
    peer: Value,
) -> (
    ws_login::Request,
    plain_rs::features::services::confirmation_channel::Channel,
) {
    use plain_rs::features::services::confirmation_channel::{Client, ServerHandshake};
    let (login, hello) = Client::start("browser").unwrap();
    let signing =
        plain_rs::features::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
    let now = plain_libs::chrono::Utc::now().timestamp_millis() as u64;
    let device_id = state.native_client_id().unwrap();
    let (channel, response) =
        ServerHandshake::start("browser", &device_id, &hello, &signing, now).unwrap();
    let keys = login
        .accept(
            &response,
            &plain_libs::utils::base64::base64_encode(&signing[32..]),
            &device_id,
            now,
        )
        .unwrap();
    let request = json!({"appName":"Test","appVersion":"1","osName":"Fixture","osVersion":"1","isMobile":false,"peer":peer});
    let frame = serde_json::to_vec(&request).unwrap();
    let frame = keys.encrypt_request(&frame, now).unwrap();
    (
        ws_login::Request::Issue {
            client_id: "browser".into(),
            client_ip: "192.0.2.1".into(),
            frame: plain_libs::utils::base64::base64_encode(&frame),
            channel: ws_login::LoginChannel::Confirmation(channel),
        },
        keys,
    )
}

#[tokio::test]
async fn approved_web_login_encrypted_response_and_token_auth_survive_restart() {
    let (dir, server) = fixture();
    let state = server.runtime_state();
    let (request, keys) = offer(&state, Value::Null);
    let pending = ws_login::execute(&state, request).await.unwrap();
    assert_eq!(pending["status"], "PENDING");
    let issued = ws_login::execute(
        &state,
        ws_login::Request::Complete {
            request_id: pending["requestId"].as_str().unwrap().to_owned(),
        },
    )
    .await
    .unwrap();
    assert_eq!(issued["status"], "COMPLETED");
    let shared = keys.api_key();
    assert_eq!(
        plain_libs::utils::base64::base64_encode(&shared),
        issued["token"]
    );
    let row = state.db.session_get("browser").unwrap().unwrap();
    assert_eq!(row.r#type, plain_rs::features::sessions::SessionType::Login);
    assert_eq!(
        plain_rs::infrastructure::db::Db::open(
            &plain_rs::infrastructure::data_paths::DataPaths::new(dir.path()).database()
        )
        .unwrap()
        .session_get("browser")
        .unwrap()
        .unwrap()
        .token,
        row.token
    );
    let encrypted = keys
        .decrypt_response(
            &plain_libs::utils::base64::base64_decode(issued["frame"].as_str().unwrap()),
            plain_libs::chrono::Utc::now().timestamp_millis() as u64,
        )
        .unwrap();
    let decrypted = encrypted;
    assert_eq!(
        serde_json::from_slice::<Value>(&decrypted).unwrap(),
        issued["response"]
    );
    assert!(issued["response"].get("ecdhPublicKey").is_none());
    assert!(issued["response"].get("signature").is_none());
    let mut headers = HeaderMap::new();
    headers.insert("c-id", "browser".parse().unwrap());
    let plaintext = format!(
        "{}|{}|{{\"query\":\"{{noteCount(query:\\\"\\\")}}\"}}",
        plain_libs::chrono::Utc::now().timestamp_millis(),
        plain_libs::utils::uuid::Uuid::new_v4()
    );
    let encrypted = plain_rs::infrastructure::transport::http_request::encrypt_raw(
        &shared,
        "POST",
        "/graphql",
        plaintext.as_bytes(),
    )
    .unwrap();
    let context = plain_rs::infrastructure::transport::http_response::Context::new(&encrypted);
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
    let pending = ws_login::execute(&state, offer(&state, Value::Null).0)
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
        let pending = ws_login::execute(&state, offer(&state, Value::Null).0)
            .await
            .unwrap();
        let id = pending["requestId"].as_str().unwrap().to_owned();
        if cancelled {
            ws_login::execute(
                &state,
                ws_login::Request::Cancel {
                    reason: "cancelled",
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
    assert_eq!(plain_libs::utils::base64::base64_decode(token).len(), 32);
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
    let reopened = plain_rs::infrastructure::db::Db::open(
        &plain_rs::infrastructure::data_paths::DataPaths::new(dir.path()).database(),
    )
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
    assert_eq!(
        state
            .db
            .session_list()
            .unwrap()
            .iter()
            .map(|session| session.client_id.as_str())
            .collect::<Vec<_>>(),
        vec!["phone"]
    );
    let unauthorized = state.public.execute("{ sessions { clientId } }").await;
    assert!(!unauthorized.errors.is_empty());
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn login_pairing_and_init_preserve_rust_state_without_revealing_passwords() {
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    let (_, public) = plain_libs::crypto::ed25519_generate();
    let peer = json!({"port":8443,"signaturePublicKey":plain_libs::utils::base64::base64_encode(&public),"deviceName":"Desktop","deviceType":"COMPUTER","ips":["192.0.2.1"]});
    let pending = ws_login::execute(&state, offer(&state, peer).0)
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
            b"chat-pairing".as_slice(),
            plain_libs::utils::base64::base64_decode(issued["token"].as_str().unwrap()).as_slice(),
        ]
        .concat(),
    );
    assert_eq!(
        paired.key,
        plain_libs::utils::base64::base64_encode(&expected)
    );
    assert_eq!(paired.name, "Desktop");
    // Changing the login mode must not revoke the already approved API session.
    state.prefs.set("password_type", "FIXED").unwrap();
    let token = key(&state.db, "browser").unwrap().unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("c-id", "browser".parse().unwrap());
    let response = crate::http::init::init(
        State(state.clone()),
        axum::extract::ConnectInfo("127.0.0.1:1".parse().unwrap()),
        headers.clone(),
        axum::body::Bytes::from(
            plain_rs::infrastructure::transport::http_request::encrypt(&token, "/init", "init")
                .unwrap(),
        ),
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
    assert!(
        plain_rs::features::prefs::security::opaque_public_key(&state.prefs)
            .unwrap()
            .is_some()
    );
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn main_schema_keeps_secrets_local_and_remote_metadata_and_resource_token_available() {
    use plain_rs::infrastructure::db::chat_store::{SaveMode, channels, peers};
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    let session = create(&state, "Remote".into()).unwrap();
    state.prefs.set("adb_token", "adb-secret").unwrap();
    state.prefs.set("dev_token", "dev-secret").unwrap();
    let mut peer = plain_rs::infrastructure::db::DPeer::new(
        "peer",
        "Peer",
        "192.0.2.2",
        Some(8080),
        plain_rs::features::chat::enums::DeviceType::Tablet,
    );
    peer.key = "peer-secret".into();
    peer.token = "peer-token".into();
    peers::save(&state.db, &[peer], SaveMode::Insert).unwrap();
    let mut channel = plain_rs::infrastructure::db::DChannel::new("Channel", "peer");
    channel.id = "channel".into();
    channel.key = "channel-secret".into();
    channels::save(&state.db, &[channel], SaveMode::Insert).unwrap();
    let native = server.native_graphql().unwrap();
    let query = r#"{ sessions { clientId token } peers { key token } chatChannels { key } }"#;
    let local = native.execute(query.into()).await;
    assert!(local.errors.is_empty(), "{:?}", local.errors);
    let local = local.data.into_json().unwrap();
    let settings = server.native_management().unwrap().preferences().unwrap();
    assert_eq!(settings["password"], "");
    assert!(settings.get("url_token").is_none());
    assert_eq!(local["peers"][0]["key"], "peer-secret");
    assert_eq!(local["chatChannels"][0]["key"], "channel-secret");
    assert_eq!(
        local["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["clientId"] == session.client_id)
            .unwrap()["token"],
        session.token
    );
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
        metadata["data"]["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["clientId"] == session.client_id)
            .unwrap()["clientId"],
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
