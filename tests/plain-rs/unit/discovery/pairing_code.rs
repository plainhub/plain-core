use super::*;

#[test]
fn pairing_code_preserves_identity_and_decodes_utf8_plus_and_repeated_addresses() {
    let value = parse("plainapp://pair?v=1&id=fixture&name=%E4%B8%AD%E6%96%87+device&ips=192.0.2.1,192.0.2.2&ips=2001%3Adb8%3A%3A1&port=8443").unwrap();
    assert_eq!(value.name, "中文 device");
    assert_eq!(value.ips, ["192.0.2.1", "192.0.2.2", "2001:db8::1"]);
    let device = value.into_device();
    assert_eq!(device.id, "fixture");
    assert_eq!(device.port, 8443);
    assert_eq!(device.device_type, "COMPUTER");
    assert_eq!(device.discovery_methods, ["LAN", "QR"]);
    device.validate().unwrap();
}

#[test]
fn non_pairing_codes_and_incomplete_or_out_of_range_targets_are_rejected() {
    for code in [
        "hello",
        "https://example.com/pair?v=1",
        "plainapp://pair?v=1&name=Device&ips=192.0.2.1&port=8443",
        "plainapp://pair?id=fixture&name=Device&ips=&port=8443",
        "plainapp://pair?id=fixture&name=Device&ips=192.0.2.1&port=invalid",
        "plainapp://pair?id=fixture&name=Device&ips=192.0.2.1&port=65536",
        "plainapp://pair?id=fixture&name=Device&ips=192.0.2.1&port=0",
    ] {
        assert!(parse(code).is_none(), "{code}");
    }
}

#[test]
fn repeated_scalar_fields_follow_the_last_value() {
    let value =
        parse("plainapp://pair?id=old&id=current&name=Device&ips=192.0.2.1&port=80&port=443")
            .unwrap();
    assert_eq!(value.id, "current");
    assert_eq!(value.port, 443);
}

#[test]
fn generated_codes_round_trip_reserved_characters_and_ipv6_without_platform_logic() {
    let value = Payload {
        id: "id+&value".into(),
        name: "Device α + &? /".into(),
        ips: vec!["192.0.2.4".into(), "2001:db8::1".into()],
        port: 8443,
    };
    let code = format(&value).unwrap();
    assert_eq!(parse(&code).unwrap(), value);
    assert!(code.contains("name=Device%20%CE%B1%20%2B%20%26%3F%20%2F"));
    assert!(!code.contains("name=Device "));
    let no_listener = Payload { port: 0, ..value };
    assert!(format(&no_listener).is_none());
}
