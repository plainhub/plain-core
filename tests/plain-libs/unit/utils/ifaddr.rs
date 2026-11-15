use super::*;

#[test]
fn the_lists_are_well_formed_or_empty() {
    for interface in list() {
        assert!(!interface.ip.is_loopback());
        assert!(!interface.name.is_empty());
    }
    for interface in list_v6() {
        assert!(!interface.ip.is_loopback());
        assert!(!interface.name.is_empty());
        assert!(!is_link_local(&interface.ip));
    }
}

#[cfg(unix)]
#[test]
fn a_v6_index_matches_what_the_kernel_reports_for_that_name() {
    for interface in interfaces().1 {
        let name = std::ffi::CString::new(interface.name).unwrap();
        assert_eq!(interface.index, unsafe {
            libc::if_nametoindex(name.as_ptr())
        });
    }
}

#[test]
fn address_lists_preserve_multicast_support_without_filtering_unicast_interfaces() {
    use crate::platform::network::{AddressV4, AddressV6, Interface};
    for multicast in [false, true] {
        let (v4, v6) = addresses(vec![Interface {
            name: "test-interface".into(),
            up: true,
            multicast,
            ipv4: vec![AddressV4 {
                ip: "192.0.2.1".parse().unwrap(),
                netmask: Some("255.255.255.0".parse().unwrap()),
            }],
            ipv6: vec![AddressV6 {
                ip: "2001:db8::1".parse().unwrap(),
                index: 7,
            }],
            ..Default::default()
        }]);
        assert_eq!(v4.len(), 1);
        assert_eq!(v6.len(), 1);
        assert_eq!(v4[0].multicast, multicast);
        assert_eq!(v6[0].multicast, multicast);
    }
}
