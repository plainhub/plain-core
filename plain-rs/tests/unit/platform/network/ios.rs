use super::*;

#[test]
fn apple_interface_names_preserve_wireless_wired_and_tunnel_facts() {
    assert_eq!(interface_type("en0"), InterfaceType::Wireless80211);
    for name in ["en1", "bridge0"] {
        assert_eq!(interface_type(name), InterfaceType::Ethernet);
    }
    for name in ["utun0", "ipsec3"] {
        assert_eq!(interface_type(name), InterfaceType::Tunnel);
    }
    assert_eq!(interface_type("ppp0"), InterfaceType::Ppp);
    for name in ["pdp_ip0", "lo0"] {
        assert_eq!(interface_type(name), InterfaceType::Unknown);
    }
}
