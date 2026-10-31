use super::*;
use futures_util::{SinkExt, StreamExt};
use plain_server::server::ContentServer;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

async fn frame(socket: &mut Socket) -> Message {
    tokio::time::timeout(DEADLINE, socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}

async fn browser_login(
    core: &ContentServer,
    port: u16,
    client_id: &str,
) -> (
    Socket,
    plain_rs::services::confirmation_channel::Channel,
    plain_rs::crypto::EcdhSession,
) {
    use plain_rs::services::confirmation_channel::{Client, ConfirmationMode};
    let (mut socket, _) = tokio_tungstenite::connect_async(format!(
        "ws://127.0.0.1:{port}/auth?cid={client_id}&auth=2"
    ))
    .await
    .unwrap();
    let (client, hello) = Client::start(client_id, ConfirmationMode::DeviceConfirmation).unwrap();
    socket
        .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
        .await
        .unwrap();
    let Message::Text(hello) = frame(&mut socket).await else {
        panic!("signed hello required")
    };
    let state = core.runtime();
    let public = plain_rs::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
    let (channel, reveal) = client
        .accept(
            &serde_json::from_str(&hello).unwrap(),
            &plain_rs::utils::base64::base64_encode(&public[32..]),
            &state.native_client_id().unwrap(),
            now_ms(),
        )
        .unwrap();
    socket
        .send(Message::Text(
            serde_json::to_string(&reveal).unwrap().into(),
        ))
        .await
        .unwrap();
    let session = plain_rs::crypto::EcdhSession::generate();
    let request = json!({"browserName":"Fixture","browserVersion":"1","osName":"Fixture","osVersion":"1","isMobile":false,"ecdhPublicKey":plain_rs::utils::base64::base64_encode(&session.public_key_bytes),"loginContext":channel.context});
    let encrypted = channel
        .encrypt_request(&serde_json::to_vec(&request).unwrap(), now_ms())
        .unwrap();
    socket
        .send(Message::Binary(encrypted.into()))
        .await
        .unwrap();
    let Message::Binary(pending) = frame(&mut socket).await else {
        panic!("encrypted login response required")
    };
    let pending: Value =
        serde_json::from_slice(&channel.decrypt_response(&pending, now_ms()).unwrap()).unwrap();
    assert_eq!(pending["status"], "PENDING");
    (socket, channel, session)
}

#[tokio::test]
async fn local_approval_socket_controls_real_passwordless_browser_sessions() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime();
    state.prefs.set("password_type", "NONE").unwrap();
    state.prefs.set("login_require_approval", true).unwrap();
    state.prefs.set("password", "unexposed-password").unwrap();
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let (port, _) = core
        .start_public(
            0,
            0,
            cert.cert.pem().into_bytes(),
            cert.key_pair.serialize_pem().into_bytes(),
        )
        .await
        .unwrap();
    let service = Service::start(&core).await.unwrap().unwrap();
    for allowed in [false, true] {
        let client_id = if allowed {
            "local-approved"
        } else {
            "local-denied"
        };
        let (mut socket, channel, session) = browser_login(&core, port, client_id).await;
        let page = requests(directory.path(), None).await.unwrap();
        assert_eq!(page.requests.len(), 1);
        let request = &page.requests[0];
        assert_eq!(request.client_id, client_id);
        assert_eq!(
            request.verification_code.as_deref(),
            Some(channel.verification_code.as_str())
        );
        assert!(state.db.session_get(client_id).unwrap().is_none());
        decide(directory.path(), request.request_id.clone(), allowed)
            .await
            .unwrap();
        let response = frame(&mut socket).await;
        if allowed {
            let Message::Binary(bytes) = response else {
                panic!("encrypted completion required")
            };
            let completed: Value =
                serde_json::from_slice(&channel.decrypt_response(&bytes, now_ms()).unwrap())
                    .unwrap();
            assert_eq!(completed["status"], "COMPLETED");
            let shared = session
                .compute_shared_key(&plain_rs::utils::base64::base64_decode(
                    completed["ecdhPublicKey"].as_str().unwrap(),
                ))
                .unwrap();
            assert_eq!(
                state.db.session_get(client_id).unwrap().unwrap().token,
                plain_rs::utils::base64::base64_encode(&shared)
            );
        } else {
            assert!(matches!(response, Message::Close(Some(close)) if close.reason == "rejected"));
            assert!(state.db.session_get(client_id).unwrap().is_none());
        }
        assert!(
            requests(directory.path(), None)
                .await
                .unwrap()
                .requests
                .is_empty()
        );
        assert!(
            decide(directory.path(), request.request_id.clone(), true)
                .await
                .is_err()
        );
    }
    core.stop_core().await;
    service.shutdown().await.unwrap();
    core.shutdown().await;
    assert!(requests(directory.path(), None).await.is_err());
}
