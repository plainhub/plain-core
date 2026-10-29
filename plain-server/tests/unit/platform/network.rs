use super::*;

fn select_lan_ipv4(interfaces: &[Interface]) -> String {
    select_lan_ipv4_addresses(interfaces)
        .into_iter()
        .next()
        .unwrap_or_default()
}

fn interface(kind: InterfaceType, addresses: &[&str]) -> Interface {
    Interface {
        if_type: kind,
        ipv4: addresses
            .iter()
            .map(|value| plain_rs::platform::network::AddressV4 {
                ip: value.split('/').next().unwrap().parse().unwrap(),
                netmask: None,
            })
            .collect(),
        up: true,
        loopback: false,
        tunnel: false,
        ..Default::default()
    }
}

#[test]
fn lan_selection_uses_one_shared_wireless_ethernet_fallback_order() {
    let other = interface(InterfaceType::Unknown, &["192.0.2.1/24"]);
    let ethernet = interface(InterfaceType::Ethernet, &["198.51.100.1/24"]);
    let wireless = interface(InterfaceType::Wireless80211, &["203.0.113.1/24"]);
    assert_eq!(
        select_lan_ipv4(&[other.clone(), ethernet.clone(), wireless]),
        "203.0.113.1"
    );
    assert_eq!(select_lan_ipv4(&[other.clone(), ethernet]), "198.51.100.1");
    assert_eq!(select_lan_ipv4(&[other]), "192.0.2.1");
    assert_eq!(select_lan_ipv4(&[]), "");
}

#[test]
fn lan_selection_excludes_down_loopback_tunnel_and_unusable_addresses() {
    let mut down = interface(InterfaceType::Wireless80211, &["203.0.113.1/24"]);
    down.up = false;
    let mut loopback = interface(InterfaceType::Wireless80211, &["203.0.113.2/24"]);
    loopback.loopback = true;
    let mut tunnel = interface(InterfaceType::Unknown, &["203.0.113.3/24"]);
    tunnel.tunnel = true;
    let declared_tunnel = interface(InterfaceType::Tunnel, &["203.0.113.4/24"]);
    let ppp = interface(InterfaceType::Ppp, &["203.0.113.5/24"]);
    let invalid = interface(
        InterfaceType::Wireless80211,
        &[
            "0.0.0.0/0",
            "127.0.0.1/8",
            "169.254.1.1/16",
            "224.0.0.1/4",
            "255.255.255.255/32",
        ],
    );
    let ethernet = interface(InterfaceType::Ethernet, &["198.51.100.1/24"]);
    let mut candidates = vec![down, loopback, tunnel, declared_tunnel, ppp, invalid];
    assert_eq!(select_lan_ipv4(&candidates), "");
    candidates.push(ethernet);
    assert_eq!(select_lan_ipv4(&candidates), "198.51.100.1");
}

#[test]
fn lan_selection_retains_sample_order_between_equal_priority_interfaces() {
    let first = interface(
        InterfaceType::Wireless80211,
        &["203.0.113.1/24", "203.0.113.2/24"],
    );
    let second = interface(InterfaceType::Wireless80211, &["198.51.100.1/24"]);
    assert_eq!(
        select_lan_ipv4(&[first.clone(), second.clone()]),
        "203.0.113.1"
    );
    assert_eq!(select_lan_ipv4(&[second, first]), "198.51.100.1");
}

#[test]
fn network_sample_deduplicates_addresses_without_including_tunnels() {
    let wireless = interface(
        InterfaceType::Wireless80211,
        &["203.0.113.1/24", "203.0.113.1/24"],
    );
    let tunnel = interface(InterfaceType::Tunnel, &["198.51.100.1/24"]);
    assert_eq!(
        select_lan_ipv4_addresses(&[wireless, tunnel]),
        ["203.0.113.1"]
    );
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[test]
fn default_interface_classification_keeps_absence_distinct_from_other_networks() {
    for (kind, expected) in [
        (Some(InterfaceType::Wireless80211), NetworkType::Wifi),
        (Some(InterfaceType::Ethernet), NetworkType::Ethernet),
        (Some(InterfaceType::Cellular), NetworkType::Cellular),
        (Some(InterfaceType::Tunnel), NetworkType::Other),
        (Some(InterfaceType::Ppp), NetworkType::Other),
        (Some(InterfaceType::Unknown), NetworkType::Other),
        (None, NetworkType::None),
    ] {
        assert_eq!(classify_network(Ok(kind)).unwrap(), expected);
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[test]
fn native_query_errors_are_reported_instead_of_becoming_no_network() {
    let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "network query denied");
    assert_eq!(
        classify_network(Err(error)).unwrap_err(),
        "network query denied"
    );
}
