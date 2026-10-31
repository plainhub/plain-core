//! Advertisement facts read through the shared core. This is reached from
//! the mDNS and pairing runtimes, not from its own HTTP route, so it is
//! exercised through `execute` rather than a listener.

use plain_rs::runtime::Runtime as ServerState;

use plain_rs::services::discovery::discovery_advertisement::*;

fn core_with_host(directory: &std::path::Path) -> (crate::server::ContentServer, ServerState) {
    let prefs =
        std::sync::Arc::new(plain_rs::prefs::Prefs::load(&directory.join("system.json")).unwrap());
    prefs.set("client_id", "fixture-root").unwrap();
    let core = crate::server::ContentServer::start_core(
        &directory,
        &plain_rs::utils::base64::base64_encode(&[9; 32]),
        prefs,
    )
    .unwrap();
    let state = core.runtime_state().core;
    (core, state)
}

#[tokio::test]
async fn first_start_advertises_the_persisted_generated_mdns_name() {
    let _runtime = plain_rs::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let directory = tempfile::tempdir().unwrap();
    let (core, state) = core_with_host(directory.path());
    let hostname = plain_rs::prefs::security::snapshot(&state.prefs)
        .unwrap()
        .mdns_hostname;
    let label = hostname.strip_suffix(".local").unwrap();
    assert_eq!(label.len(), 2);
    assert!(
        label
            .bytes()
            .all(|byte| b"abcdefghjkmnpqrstuwxyz".contains(&byte))
    );
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    core.start_public(
        0,
        0,
        cert.cert.pem().into_bytes(),
        cert.key_pair.serialize_pem().into_bytes(),
    )
    .await
    .unwrap();
    let advertised = execute(&state, Request::Mdns {}).await.unwrap();
    assert_eq!(advertised["targetHostname"], hostname);
    let reloaded = plain_rs::prefs::Prefs::load(state.prefs.path()).unwrap();
    assert_eq!(plain_rs::prefs::ensure_mdns_hostname(&reloaded), hostname);
    state
        .prefs
        .set("mdns_hostname", "custom-device.local")
        .unwrap();
    let advertised = execute(&state, Request::Mdns {}).await.unwrap();
    assert_eq!(advertised["targetHostname"], "custom-device.local");
    core.stop_core().await;
}
