use super::*;

#[test]
fn parse_response_skips_authority_section() {
    let mut out = Vec::new();
    write_u16(&mut out, 0);
    write_u16(&mut out, DNS_RESPONSE_FLAGS);
    write_u16(&mut out, 0);
    write_u16(&mut out, 1);
    write_u16(&mut out, 1);
    write_u16(&mut out, 1);
    write_record(
        &mut out,
        &encode_name("_plainapp._tcp.local"),
        TYPE_PTR,
        DNS_CLASS_IN,
        TTL_SECONDS,
        &encode_name("X._plainapp._tcp.local"),
    );
    write_record(
        &mut out,
        &encode_name("junk.local"),
        TYPE_A,
        DNS_CLASS_IN,
        TTL_SECONDS,
        &[1, 2, 3, 4],
    );
    write_record(
        &mut out,
        &encode_name("p9.local"),
        TYPE_A,
        DNS_CACHE_FLUSH_CLASS_IN,
        TTL_SECONDS,
        &[192, 168, 1, 20],
    );

    let parsed = parse_response(&out).expect("parse");
    assert_eq!(parsed.answers.len(), 1);
    assert_eq!(parsed.additional.len(), 1);
    assert_eq!(parsed.additional[0].name, "p9.local");
    assert_eq!(parsed.additional[0].ip().unwrap(), "192.168.1.20");
}

#[test]
fn ipv6_to_string_compresses_longest_zero_run() {
    let v = |vals: &[u16]| {
        let mut b = Vec::new();
        for x in vals {
            b.push((x >> 8) as u8);
            b.push((x & 0xFF) as u8);
        }
        ipv6_to_string(&b).expect("ipv6")
    };
    assert_eq!(
        v(&[0xfe80, 0, 0, 0, 0, 0xcc7, 0x94ea, 0xcb1]),
        "fe80::cc7:94ea:cb1"
    );
    assert_eq!(v(&[0x2001, 0xdb8, 0, 0, 0, 0, 0, 1]), "2001:db8::1");
    assert_eq!(v(&[0, 0, 0, 0, 0, 0, 0, 1]), "::1");
    assert_eq!(v(&[1, 2, 3, 4, 5, 6, 7, 8]), "1:2:3:4:5:6:7:8");
    assert!(ipv6_to_string(&[1, 2, 3, 4]).is_none());
}

#[test]
fn build_query_qu_sets_unicast_response_bit() {
    let qu = build_query("_plainapp._tcp.local", TYPE_PTR, true);
    let question = &read_questions(&qu).expect("questions")[0];
    assert_eq!(question.qclass, DNS_CLASS_IN);
    assert!(question.unicast_response_requested);

    let qm = build_query("_plainapp._tcp.local", TYPE_PTR, false);
    assert!(!read_questions(&qm).expect("questions")[0].unicast_response_requested);
}

#[test]
fn parse_response_tolerates_question_section() {
    // Legacy-unicast (QU) responses echo the question before the answers
    // (RFC 6762 §6.7); the browser must still see the records.
    let mut out = Vec::new();
    write_u16(&mut out, 0);
    write_u16(&mut out, DNS_RESPONSE_FLAGS);
    write_u16(&mut out, 1);
    write_u16(&mut out, 1);
    write_u16(&mut out, 0);
    write_u16(&mut out, 0);
    out.extend_from_slice(&encode_name("_plainapp._tcp.local"));
    write_u16(&mut out, TYPE_PTR);
    write_u16(&mut out, DNS_CLASS_IN);
    write_record(
        &mut out,
        &encode_name("_plainapp._tcp.local"),
        TYPE_PTR,
        DNS_CLASS_IN,
        TTL_SECONDS,
        &encode_name("Pixel 7._plainapp._tcp.local"),
    );

    let parsed = parse_response(&out).expect("parse");
    assert!(parsed.is_response());
    assert_eq!(parsed.answers.len(), 1);
    assert_eq!(
        parsed.answers[0].ptr_target().unwrap(),
        "Pixel 7._plainapp._tcp.local"
    );
}
