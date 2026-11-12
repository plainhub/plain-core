use super::*;
#[test]
fn capture_is_bounded_and_decodes_dns_without_global_test_state() {
    let query = packet_codec::build_ptr_query("_plainapp._tcp.local");
    let (summary, detail) = decode(&query);
    assert!(summary.contains("query PTR"));
    assert!(detail.contains("_plainapp._tcp.local"));
    let mut state = Capture::default();
    for direction in [MdnsPacketDirection::In, MdnsPacketDirection::Out] {
        for time in 0..60 {
            state.push(PacketLog {
                time: crate::infrastructure::types::Instant(
                    plain_libs::chrono::DateTime::from_timestamp_millis(time).unwrap(),
                ),
                direction,
                src_ip: "127.0.0.1".into(),
                src_port: 5353,
                dst_ip: "224.0.0.251".into(),
                dst_port: 5353,
                size: query.len(),
                summary: summary.clone(),
                detail: detail.clone(),
            });
        }
    }
    assert_eq!(state.inbound.len(), 50);
    assert_eq!(state.outbound.len(), 50);
    assert_eq!(state.inbound.front().unwrap().time.0.timestamp_millis(), 59);
    assert_eq!(state.inbound.back().unwrap().time.0.timestamp_millis(), 10);
    assert_eq!(decode(&[0xff]).1, "(unparseable)");
}

#[test]
fn packet_directions_keep_fixed_wire_values_and_reject_unknown_values() {
    for (direction, wire) in [
        (MdnsPacketDirection::In, "IN"),
        (MdnsPacketDirection::Out, "OUT"),
    ] {
        assert_eq!(plain_libs::serde_json::to_value(direction).unwrap(), wire);
        assert_eq!(
            plain_libs::serde_json::from_value::<MdnsPacketDirection>(
                plain_libs::serde_json::json!(wire)
            )
            .unwrap(),
            direction
        );
    }
    assert!(
        plain_libs::serde_json::from_value::<MdnsPacketDirection>(plain_libs::serde_json::json!(
            "SIDEWAYS"
        ))
        .is_err()
    );
}
