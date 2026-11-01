use crate::protocols::mdns::records::MdnsServiceInfo;
const SERVICE_TYPE: &str = "_example._tcp.local";
use super::*;

fn service(port: u16) -> MdnsServiceInfo {
    MdnsServiceInfo {
        instance_name: "Pixel 9".to_string(),
        service_type: SERVICE_TYPE.to_string(),
        target_hostname: "p9.local".to_string(),
        port,
        txt_records: vec!["id=abc".to_string()],
        ips: vec!["192.168.123.23".to_string()],
    }
}

#[test]
fn ptr_response_carries_srv_txt_a_in_additional() {
    let query = packet_codec::build_ptr_query(SERVICE_TYPE);
    let resp = build_response_if_match(&query, &service(8443)).expect("response");
    let parsed = packet_codec::parse_response(&resp.bytes).expect("parse");
    assert_eq!(parsed.answers.len(), 1);
    assert_eq!(parsed.additional.len(), 3); // SRV + TXT + A
    let srv = parsed
        .additional
        .iter()
        .find(|r| r.record_type == TYPE_SRV)
        .unwrap();
    assert_eq!(srv.srv().unwrap().port, 8443);
    let a = parsed
        .additional
        .iter()
        .find(|r| r.record_type == TYPE_A)
        .unwrap();
    assert_eq!(a.ip().unwrap(), "192.168.123.23");
}

// ── Goodbye (RFC 6762 §8.4) ─────────────────────────────────────────────
// Sent when a published instance is replaced by one with a different
// instance FQDN (device renamed): TTL=0 makes peers drop the stale records
// at once instead of listing the old name until the 120s TTL expires.

#[test]
fn goodbye_carries_ptr_srv_txt_with_zero_ttl() {
    let parsed = packet_codec::parse_response(&build_goodbye(&service(8443))).expect("parse");
    assert!(parsed.is_response());
    let types: Vec<u16> = parsed.answers.iter().map(|r| r.record_type).collect();
    assert_eq!(types, vec![TYPE_PTR, TYPE_SRV, TYPE_TXT]);
    for record in &parsed.answers {
        assert_eq!(record.ttl, 0);
    }
    assert_eq!(
        parsed.answers[0].ptr_target().unwrap(),
        service(8443).instance_fqdn()
    );
}

#[test]
fn live_records_parse_with_the_configured_ttl() {
    let query = packet_codec::build_ptr_query(SERVICE_TYPE);
    let resp = build_response_if_match(&query, &service(8443)).expect("response");
    let parsed = packet_codec::parse_response(&resp.bytes).expect("parse");
    for record in parsed.all_records() {
        assert_eq!(record.ttl, TTL_SECONDS);
    }
}
