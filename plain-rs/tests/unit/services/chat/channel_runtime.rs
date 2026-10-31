use super::*;
use plain_rs::db::chat_store::channels;

fn fixture() -> (tempfile::TempDir, crate::test_runtime::Core) {
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    let (key, _public) = plain_rs::crypto::ed25519_generate();
    prefs.set("device_name", "Actual tablet").unwrap();
    prefs.set("client_id", "actor").unwrap();
    prefs
        .set(
            "signature_key_pair",
            plain_rs::utils::base64::base64_encode(&key),
        )
        .unwrap();
    let server = crate::test_runtime::Core::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    (dir, server)
}
#[tokio::test]
async fn http_mutations_own_identity_pending_members_and_owner_policy() {
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    let _client = reqwest::Client::new();
    // The channel surface is GraphQL-only now; the runtime takes the actor
    // from the core identity rather than a request header.
    let result = execute(
        &state,
        Request::Create {
            name: " fixture ".into(),
        },
    )
    .await
    .unwrap();
    let id = result["channel"]["id"].as_str().unwrap();
    assert_eq!(result["channel"]["owner_id"], "actor");
    execute(
        &state,
        Request::Rename {
            id: id.into(),
            name: "renamed".into(),
        },
    )
    .await
    .unwrap();
    execute(
        &state,
        Request::Invite {
            id: id.into(),
            peer: "unknown".into(),
        },
    )
    .await
    .unwrap();
    let channel = get(&state, id).unwrap();
    assert_eq!(channel.version, 3);
    assert!(channel.members.contains("PENDING"));
    assert!(
        execute(
            &state,
            Request::Resend {
                id: id.into(),
                peer: "unknown".into()
            }
        )
        .await
        .is_err()
    );
    assert!(
        execute(&state, Request::Leave { id: id.into() })
            .await
            .is_err()
    );
    execute(&state, Request::Delete { id: id.into() })
        .await
        .unwrap();
    assert!(channels::get(&state.db, id).unwrap().is_none());
    server.shutdown().await;
}
