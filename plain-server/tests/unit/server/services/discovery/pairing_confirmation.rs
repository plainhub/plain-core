use plain_rs::peers::pairing::sessions::Target;
use plain_rs::services::discovery::pairing;
use plain_rs::services::discovery::pairing_attempts::Mode;
use plain_rs::utils::base64::base64_encode;
use sha2::Digest;
use std::sync::Arc;

fn fixture(id: &str) -> (tempfile::TempDir, crate::server::ContentServer) {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("system.json")).unwrap());
    let (key, _) = plain_rs::crypto::ed25519_generate();
    prefs.set("client_id", id).unwrap();
    prefs
        .set("signature_key_pair", base64_encode(&key))
        .unwrap();
    let server =
        crate::server::ContentServer::start(directory.path(), &base64_encode(&[3; 32]), prefs)
            .unwrap();
    (directory, server)
}
fn device() -> pairing::Device {
    pairing::Device {
        name: "Fixture".into(),
        port: 2443,
        device_type: "PHONE".into(),
        ips: vec![],
        aware_supported: false,
    }
}
#[tokio::test]
async fn tls_peers_exchange_matching_codes_before_approval_and_commit_the_authenticated_response() {
    use plain_rs::services::discovery::pairing_runtime;
    use plain_rs::services::discovery::pairing_runtime::Request;
    let (_local_dir, local_server) = fixture("local");
    let (_remote_dir, remote_server) = fixture("remote");
    let mut ports = Vec::new();
    for server in [&local_server, &remote_server] {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let (_, port) = server
            .start_public(
                0,
                0,
                cert.cert.pem().into_bytes(),
                cert.key_pair.serialize_pem().into_bytes(),
            )
            .await
            .unwrap();
        ports.push(port);
    }
    let local = local_server.runtime_state().core;
    let remote = remote_server.runtime_state().core;
    let mut incoming = remote.events.subscribe();
    let (attempt, _, _) = local
        .pairing_runtime
        .attempts
        .begin(Some(&local), Some("remote".into()), None, Mode::Chat)
        .unwrap();
    let mut sender = device();
    sender.port = ports[0];
    pairing_runtime::execute(
        &local,
        Request::StartLan {
            target: Target {
                device_id: "remote".into(),
                device_name: "Remote".into(),
                device_ip: String::new(),
                device_port: ports[1],
            },
            ips: vec!["127.0.0.1".into()],
            device: sender,
            generation: Some(attempt.generation.clone()),
        },
    )
    .await
    .unwrap();
    let notice = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let event = incoming.recv().await.unwrap();
            if event.event_type == plain_rs::chat::events::WS_PAIRING_REQUEST_RECEIVED {
                break serde_json::from_str::<serde_json::Value>(&event.payload).unwrap();
            }
        }
    })
    .await
    .unwrap();
    let code = local
        .pairing_runtime
        .attempts
        .get(&attempt.generation)
        .unwrap()
        .verification_code
        .unwrap();
    assert_eq!(notice["verificationCode"], code);
    assert!(
        plain_rs::db::chat_store::peers::get(&local.db, "remote")
            .unwrap()
            .is_none()
    );
    assert!(
        plain_rs::db::chat_store::peers::get(&remote.db, "local")
            .unwrap()
            .is_none()
    );
    let request = serde_json::from_value(notice).unwrap();
    let mut receiver = device();
    receiver.port = ports[1];
    pairing_runtime::execute(
        &remote,
        Request::Respond {
            request,
            accepted: true,
            device: receiver,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        local
            .pairing_runtime
            .attempts
            .get(&attempt.generation)
            .unwrap()
            .phase,
        "SUCCEEDED"
    );
    assert_eq!(
        plain_rs::db::chat_store::peers::get(&local.db, "remote")
            .unwrap()
            .unwrap()
            .key,
        plain_rs::db::chat_store::peers::get(&remote.db, "local")
            .unwrap()
            .unwrap()
            .key
    );
    local_server.shutdown().await;
    remote_server.shutdown().await;
}
