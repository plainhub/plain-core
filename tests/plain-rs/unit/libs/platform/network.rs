use super::*;

#[test]
fn default_interface_matching_accepts_each_address_family_and_secondary_addresses() {
    let interface = Interface {
        ipv4: vec![
            AddressV4 {
                ip: "192.0.2.1".parse().unwrap(),
                netmask: None,
            },
            AddressV4 {
                ip: "192.0.2.2".parse().unwrap(),
                netmask: None,
            },
        ],
        ipv6: vec![AddressV6 {
            ip: "2001:db8::1".parse().unwrap(),
            index: 0,
        }],
        ..Default::default()
    };
    for value in ["192.0.2.1", "192.0.2.2", "2001:db8::1"] {
        assert!(owns(&interface, value.parse().unwrap()));
    }
    for value in ["192.0.2.3", "2001:db8::2"] {
        assert!(!owns(&interface, value.parse().unwrap()));
    }
}

#[test]
fn native_enumeration_preserves_interface_identity_and_address_families() {
    for interface in interfaces().unwrap() {
        assert!(!interface.name.is_empty());
        assert!(interface.index > 0);
    }
}

#[test]
fn route_selection_prefers_ipv4_and_uses_ipv6_only_when_necessary() {
    let mut calls = Vec::new();
    let address = default_address(|bind, remote| {
        calls.push((bind.to_owned(), remote.to_owned()));
        Some("192.0.2.1".parse().unwrap())
    });
    assert_eq!(address, Some("192.0.2.1".parse().unwrap()));
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "0.0.0.0:0");
    calls.clear();
    let address = default_address(|bind, remote| {
        calls.push((bind.to_owned(), remote.to_owned()));
        bind.starts_with('[')
            .then(|| "2001:db8::1".parse().unwrap())
    });
    assert_eq!(address, Some("2001:db8::1".parse().unwrap()));
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].0, "[::]:0");
    let mut count = 0;
    assert_eq!(
        default_address(|_, _| {
            count += 1;
            None
        }),
        None
    );
    assert_eq!(count, 2);
}
