use super::*;
use plain_libs::utils::ifaddr::{Ifv4, Ifv6};

#[test]
fn ipv4_candidates_use_multicast_flags_before_address_deduplication() {
    for (excluded, included) in [("test-tunnel", "test-lan"), ("test-lan", "test-tunnel")] {
        let interface = |name: &str, multicast| Ifv4 {
            name: name.into(),
            multicast,
            ip: "192.0.2.1".parse().unwrap(),
            netmask: Some("255.255.255.0".parse().unwrap()),
        };
        let candidates = ipv4_interfaces(vec![
            interface(excluded, false),
            interface(included, true),
            interface("test-duplicate", true),
        ]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, included);
        assert!(ipv4_interfaces(vec![interface(excluded, false)]).is_empty());
    }
}

#[test]
fn ipv6_candidates_use_multicast_flags_before_index_deduplication() {
    for (excluded, included) in [("test-tunnel", "test-lan"), ("test-lan", "test-tunnel")] {
        let interface = |name: &str, multicast| Ifv6 {
            name: name.into(),
            multicast,
            ip: "2001:db8::1".parse().unwrap(),
            index: 7,
        };
        assert_eq!(
            ipv6_interfaces(vec![
                interface(excluded, false),
                interface(included, true),
                interface("test-duplicate", true),
            ]),
            vec![(included.to_string(), 7)]
        );
        assert!(ipv6_interfaces(vec![interface(excluded, false)]).is_empty());
    }
}

#[test]
fn failed_interface_selection_sends_no_packet_and_next_selection_can_succeed() {
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let receiver = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_nonblocking(true).unwrap();
    let target = receiver.local_addr().unwrap();
    let payload = b"test-mdns-packet";
    let error = send_on_interface(&socket, payload, target, || {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "test-selection-failure",
        ))
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(error.to_string().contains("select multicast interface"));
    let mut bytes = [0; 64];
    assert_eq!(
        receiver.recv_from(&mut bytes).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    let mut selections = 0;
    assert_eq!(
        send_on_interface(&socket, payload, target, || {
            selections += 1;
            Ok(())
        })
        .unwrap(),
        payload.len()
    );
    assert_eq!(selections, 1);
    receiver.set_nonblocking(false).unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let (size, _) = receiver.recv_from(&mut bytes).unwrap();
    assert_eq!(&bytes[..size], payload);
    receiver.set_nonblocking(true).unwrap();
    assert_eq!(
        receiver.recv_from(&mut bytes).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}
