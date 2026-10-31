use super::*;

#[test]
fn all_address_fields_reuse_the_supplied_interface_snapshot() {
    let peer = DPeer::new(
        "fixture",
        "Fixture",
        "192.0.2.5,198.51.100.5",
        Some(8443),
        plain_rs::chat::enums::DeviceType::Computer,
    );
    let interfaces = vec![plain_rs::transport::lan::Interface {
        ip: "198.51.100.8".parse().unwrap(),
        prefix_length: 24,
    }];
    let address = address_with_interfaces(&peer, &interfaces);
    assert_eq!(address.best_ip, "198.51.100.5");
    assert_eq!(address.base_url, "https://198.51.100.5:8443");
    assert_eq!(address.api_url, "https://198.51.100.5:8443/peer_graphql");
    assert_eq!(address.status_ws_url, "wss://198.51.100.5:8443/status");
    let hostname = DPeer::new(
        "fixture",
        "Fixture",
        "device.example",
        Some(443),
        plain_rs::chat::enums::DeviceType::Computer,
    );
    assert_eq!(
        address_with_interfaces(&hostname, &[]).base_url,
        "https://device.example"
    );
}
