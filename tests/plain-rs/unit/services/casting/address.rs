use super::*;

#[test]
fn cast_http_addresses_preserve_default_port_and_validate_bound_ports() {
    assert_eq!(http_base("192.0.2.4", 80).unwrap(), "http://192.0.2.4");
    assert_eq!(
        http_base("192.0.2.4", 8080).unwrap(),
        "http://192.0.2.4:8080"
    );
    assert!(http_base("192.0.2.4", 0).is_err());
    assert!(http_base("invalid", 8080).is_err());
}

#[tokio::test]
async fn configured_device_names_need_no_network_or_host_facts() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state
        .prefs
        .set("device_name", "Configured renderer")
        .unwrap();
    let (_generation, mut requests) = state.host.connect();
    assert_eq!(device_name(&state).await.unwrap(), "Configured renderer");
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn native_default_name_never_asks_for_a_combined_address_report() {
    let host = plain_rs::host::bridge::Host::default();
    let (_generation, mut requests) = host.connect();
    assert!(!host.startup().name.is_empty());
    assert!(requests.try_recv().is_err());
}
