use super::*;

fn result(peer_id: &str, error: Option<&str>) -> ChannelDeliveryResult {
    ChannelDeliveryResult {
        peer_id: peer_id.to_string(),
        peer_name: peer_id.to_string(),
        error: error.map(|e| e.to_string()),
    }
}

#[test]
fn compute_status_aggregates() {
    assert_eq!(compute_status(&[]), ChatStatus::Sent);
    assert_eq!(
        compute_status(&[result("a", None), result("b", None)]),
        ChatStatus::Sent
    );
    assert_eq!(
        compute_status(&[result("a", Some("TIMEOUT")), result("b", Some("refused"))]),
        ChatStatus::Failed
    );
    assert_eq!(
        compute_status(&[result("a", None), result("b", Some("TIMEOUT"))]),
        ChatStatus::Partial
    );
}

#[test]
fn status_data_json_shape() {
    assert_eq!(build_status_data_json(&[]), "");
    let json = build_status_data_json(&[result("a", Some("boom")), result("b", None)]);
    let v: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&json).unwrap();
    assert_eq!(v["results"][0]["peerId"], "a");
    assert_eq!(v["results"][0]["error"], "boom");
    assert_eq!(v["results"][1]["error"], plain_libs::serde_json::Value::Null);
}
