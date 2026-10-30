use super::*;
use crate::services::discovery::{pairing, pairing_attempts::Mode};
use plain_rs::peers::pairing::security;
use plain_rs::peers::pairing::sessions::Target;
use std::sync::Arc;

fn fixture(id: &str) -> (tempfile::TempDir, crate::assembly::server::ContentServer) {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("system.json")).unwrap());
    let (key, _) = plain_rs::crypto::ed25519_generate();
    prefs.set("client_id", id).unwrap();
    prefs
        .set("signature_key_pair", base64_encode(&key))
        .unwrap();
    let server = crate::assembly::server::ContentServer::start(
        directory.path(),
        &base64_encode(&[3; 32]),
        prefs,
    )
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
fn begin(local: &ServerState, remote: &ServerState) -> (PairingRequest, Ticket, Verification) {
    let (attempt, _, _) = local
        .pairing_runtime
        .attempts
        .begin(Some(local), Some("remote".into()), None, Mode::Chat)
        .unwrap();
    let target = Target {
        device_id: "remote".into(),
        device_name: "Remote".into(),
        device_ip: "127.0.0.1".into(),
        device_port: 2443,
    };
    let value = pairing::start_generation(
        &local.prefs,
        &local.pairing,
        target,
        device(),
        Some(attempt.generation),
    )
    .unwrap();
    let request: PairingRequest = serde_json::from_value(value["request"].clone()).unwrap();
    let ticket: Ticket = serde_json::from_value(value["ticket"].clone()).unwrap();
    assert!(security::verify_request(&request));
    assert_eq!(
        pairing::receive_request(&remote.pairing, &request),
        Some(true)
    );
    remote
        .pairing_runtime
        .confirmations
        .register(&request)
        .unwrap();
    let Message::PairVerification(hello) = local
        .pairing_runtime
        .confirmations
        .begin(&request, &ticket)
        .unwrap()
    else {
        panic!()
    };
    (request, ticket, hello)
}
fn exchange(local: &ServerState, remote: &ServerState, hello: Verification) -> String {
    let Action::Notify {
        message: Message::PairVerification(challenge),
        ..
    } = remote
        .pairing_runtime
        .confirmations
        .process(remote, hello)
        .unwrap()
    else {
        panic!()
    };
    let Action::Send {
        message: Message::PairVerification(reveal),
        ..
    } = local
        .pairing_runtime
        .confirmations
        .process(local, challenge)
        .unwrap()
    else {
        panic!()
    };
    let Action::Ready { code, .. } = remote
        .pairing_runtime
        .confirmations
        .process(remote, reveal)
        .unwrap()
    else {
        panic!()
    };
    code
}
fn response(remote: &ServerState, request: PairingRequest) -> PairingResponse {
    let value = pairing::respond(
        &remote.db,
        &remote.prefs,
        &remote.pairing,
        request,
        true,
        device(),
    )
    .unwrap();
    let mut response = serde_json::from_value(value["response"].clone()).unwrap();
    remote
        .pairing_runtime
        .confirmations
        .seal(&mut response)
        .unwrap();
    response
}
#[tokio::test]
async fn same_code_and_authenticated_confirmation_precede_shared_key_commit() {
    let (_a, a) = fixture("local");
    let (_b, b) = fixture("remote");
    let local = a.runtime_state();
    let remote = b.runtime_state();
    let (request, ticket, hello) = begin(&local, &remote);
    assert!(
        remote
            .pairing_runtime
            .confirmations
            .ready(&request)
            .is_err()
    );
    let code = exchange(&local, &remote, hello);
    assert_eq!(
        local
            .pairing_runtime
            .attempts
            .get(&ticket.generation)
            .unwrap()
            .verification_code
            .as_deref(),
        Some(code.as_str())
    );
    assert_eq!(
        local
            .pairing_runtime
            .attempts
            .get(&ticket.generation)
            .unwrap()
            .phase,
        "WAITING_CONFIRMATION"
    );
    assert_eq!(code.len(), 9);
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
    remote
        .pairing_runtime
        .confirmations
        .ready(&request)
        .unwrap();
    let response = response(&remote, request);
    local
        .pairing_runtime
        .confirmations
        .response(&response)
        .unwrap();
    let completed = pairing::complete(
        &local.db,
        &local.prefs,
        &local.pairing,
        response,
        "127.0.0.1",
    )
    .unwrap();
    assert!(!completed["peer"].is_null());
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
}
#[tokio::test]
async fn forged_identity_and_changed_final_key_cannot_reuse_a_matching_code() {
    let (_a, a) = fixture("local");
    let (_b, b) = fixture("remote");
    let local = a.runtime_state();
    let remote = b.runtime_state();
    let (request, _, hello) = begin(&local, &remote);
    exchange(&local, &remote, hello);
    let mut response = response(&remote, request);
    let valid = response.clone();
    response.ecdh_public_key =
        base64_encode(&plain_rs::crypto::EcdhSession::generate().public_key_bytes);
    assert!(
        local
            .pairing_runtime
            .confirmations
            .response(&response)
            .is_err()
    );
    response = valid.clone();
    let (key, public_key) = plain_rs::crypto::ed25519_generate();
    response.signature_public_key = base64_encode(&public_key);
    response.signature = plain_rs::crypto::ed25519_sign(&key, response.signature_data().as_bytes());
    assert!(
        local
            .pairing_runtime
            .confirmations
            .response(&response)
            .is_err()
    );
    response = valid;
    response.confirmation_proof.clear();
    assert!(
        local
            .pairing_runtime
            .confirmations
            .response(&response)
            .is_err()
    );
}
#[tokio::test]
async fn cancelled_and_expired_verifications_do_not_become_ready() {
    let (_a, a) = fixture("local");
    let (_b, b) = fixture("remote");
    let local = a.runtime_state();
    let remote = b.runtime_state();
    let (request, _, hello) = begin(&local, &remote);
    let Action::Notify {
        message: Message::PairVerification(challenge),
        ..
    } = remote
        .pairing_runtime
        .confirmations
        .process(&remote, hello)
        .unwrap()
    else {
        panic!()
    };
    let Action::Send {
        message: Message::PairVerification(reveal),
        ..
    } = local
        .pairing_runtime
        .confirmations
        .process(&local, challenge)
        .unwrap()
    else {
        panic!()
    };
    remote
        .pairing
        .take_incoming(&request.from_id, &request.signature);
    assert!(
        remote
            .pairing_runtime
            .confirmations
            .process(&remote, reveal)
            .is_err()
    );
    local
        .pairing_runtime
        .confirmations
        .0
        .lock()
        .unwrap()
        .get_mut(&request.signature)
        .unwrap()
        .started = Instant::now() - RESPONSE_TIMEOUT;
    let fake = PairingResponse {
        confirmation_proof: String::new(),
        request_signature: request.signature,
        from_id: "remote".into(),
        to_id: "local".into(),
        port: 2443,
        device_type: "PHONE".into(),
        ecdh_public_key: String::new(),
        signature_public_key: String::new(),
        accepted: true,
        timestamp: plain_rs::peers::pairing::now_ms(),
        ips: vec![],
        signature: String::new(),
        aware_supported: false,
    };
    assert!(local.pairing_runtime.confirmations.response(&fake).is_err());
}

pub(crate) fn verify_incoming(state: &ServerState, request: &PairingRequest) -> String {
    let (client, hello) = Client::start(&context(request), ConfirmationMode::ChatPairing).unwrap();
    let Action::Notify {
        message:
            Message::PairVerification(Verification::Challenge {
                hello,
                signature_public_key,
                ..
            }),
        ..
    } = state
        .pairing_runtime
        .confirmations
        .process(
            state,
            Verification::Hello {
                from_id: request.from_id.clone(),
                request_signature: request.signature.clone(),
                hello,
            },
        )
        .unwrap()
    else {
        panic!()
    };
    let (channel, reveal) = client
        .accept(
            &hello,
            &signature_public_key,
            &state.native_client_id().unwrap(),
            plain_rs::peers::pairing::now_ms() as u64,
        )
        .unwrap();
    let Action::Ready {
        request: verified,
        code,
    } = state
        .pairing_runtime
        .confirmations
        .process(
            state,
            Verification::Reveal {
                from_id: request.from_id.clone(),
                request_signature: request.signature.clone(),
                reveal,
            },
        )
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(code, channel.verification_code);
    publish(state, verified, code.clone()).unwrap();
    code
}

#[tokio::test]
async fn tls_peers_exchange_matching_codes_before_approval_and_commit_the_authenticated_response() {
    use crate::services::discovery::pairing_runtime::{self, Request};
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
    let local = local_server.runtime_state();
    let remote = remote_server.runtime_state();
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
