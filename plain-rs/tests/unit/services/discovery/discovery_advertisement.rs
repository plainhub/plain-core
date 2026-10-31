//! Advertisement facts read through the shared core. This is reached from
//! the mDNS and pairing runtimes, not from its own HTTP route, so it is
//! exercised through `execute` rather than a listener.
use plain_rs::runtime::Runtime as CoreRuntime;

use super::*;

fn core_with_host(directory: &std::path::Path) -> (crate::test_runtime::Core, CoreRuntime) {
    let prefs =
        std::sync::Arc::new(plain_rs::prefs::Prefs::load(&directory.join("system.json")).unwrap());
    prefs.set("client_id", "fixture-root").unwrap();
    let core = crate::test_runtime::Core::start_core(
        &directory,
        &plain_rs::utils::base64::base64_encode(&[9; 32]),
        prefs,
    )
    .unwrap();
    let state = core.runtime_state();
    (core, state)
}

#[tokio::test]
async fn native_advertisements_use_current_preferences_without_host_calls() {
    let directory = tempfile::tempdir().unwrap();
    let (core, state) = core_with_host(directory.path());
    let (generation, mut requests) = state.host.connect();
    let first = execute(&state, Request::Reply {}).await.unwrap();
    assert_eq!(first["id"], "fixture-root");
    assert_eq!(first["deviceType"], "COMPUTER");
    assert_eq!(first["platform"], std::env::consts::OS.to_uppercase());
    state.prefs.set("device_name", "Current device").unwrap();
    state.prefs.set("https_port", 3443).unwrap();
    let second = execute(&state, Request::Reply {}).await.unwrap();
    assert_eq!(second["name"], "Current device");
    assert_eq!(second["port"], 0);
    for ip in second["ips"].as_array().unwrap() {
        assert!(ip.as_str().unwrap().parse::<std::net::Ipv4Addr>().is_ok());
    }
    assert!(requests.try_recv().is_err());
    state.host.disconnect(generation);
    core.stop_core().await;
}
