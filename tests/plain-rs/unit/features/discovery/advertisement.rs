use super::*;
fn facts() -> Facts {
    Facts {
        name: "Actual tablet".into(),
        device_type: DeviceType::Tablet,
        version: "3.3.24".into(),
        platform: "IOS".into(),
        ips: vec!["192.168.7.2".into()],
        aware_supported: true,
        aware_running: false,
    }
}
#[test]
fn same_identity_and_os_facts_assemble_all_advertisements() {
    let reply = reply("fixture", "中文.Device", 2443, facts()).unwrap();
    let service = mdns(&reply, "fixture.local").unwrap();
    assert_eq!(service.instance_name, "中文.Device");
    assert_eq!(service.service_type, "_plainapp._tcp.local");
    assert_eq!(service.target_hostname, "fixture.local");
    assert_eq!(service.port, 2443);
    assert_eq!(service.ips, reply.ips);
    assert_eq!(
        service.txt_records,
        vec![
            "id=fixture",
            "dv=TABLET",
            "ver=3.3.24",
            "pf=IOS",
            "aw=1",
            "ar=0"
        ]
    );
    assert_eq!(
        super::reply("fixture", "", 2443, facts()).unwrap().name,
        "Actual tablet"
    );
    assert!(super::reply("", "", 2443, facts()).is_err());
    let mut no_port = reply.clone();
    no_port.port = 0;
    assert!(mdns(&no_port, "fixture.local").is_err());
    assert!(mdns(&reply, "").is_err());
}
#[test]
fn uuid_only_advertisements_share_rust_filtering_and_canonicalization() {
    let id = "706c6169-2d74-8e53-9a16-f08b62c591d3";
    assert_eq!(
        decode_ble(&["180D".into(), id.to_uppercase()]),
        Some(id.into())
    );
    for services in [
        vec![],
        vec!["180D".into()],
        vec!["d8d5c4a0-8f0a-4e7d-b9e1-706c70616901".into()],
    ] {
        assert!(decode_ble(&services).is_none());
    }
    let reply = reply(id, "Device", 2443, facts()).unwrap();
    assert!(reply.aware_supported);
    assert!(!reply.aware_running);
}
