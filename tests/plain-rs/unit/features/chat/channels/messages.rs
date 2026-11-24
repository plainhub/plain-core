use super::*;

#[test]
fn members_encode_decode_roundtrip() {
    let roster = vec![ChannelMember::new("a"), ChannelMember::pending("b")];
    let json = plain_libs::serde_json::to_string(&roster).unwrap();
    assert_eq!(
        json,
        r#"[{"peerId":"a","status":"JOINED"},{"peerId":"b","status":"PENDING"}]"#
    );
    assert_eq!(decode_members(&json), roster);
    // Legacy storage/wire keys written by pre-rename app versions.
    assert_eq!(
        decode_members(r#"[{"id":"a","status":"JOINED"},{"id":"b","status":"PENDING"}]"#),
        roster
    );
}
