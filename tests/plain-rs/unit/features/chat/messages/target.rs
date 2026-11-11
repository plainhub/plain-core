use super::*;

#[test]
fn parses_direct_and_channel_targets_and_rejects_empty_ids() {
    for (encoded, to, channel) in [
        ("peer:local", "local", ""),
        ("peer:other", "other", ""),
        ("channel:group", "", "group"),
    ] {
        assert_eq!(parse(encoded).unwrap(), (to.into(), channel.into()));
    }
    for encoded in ["", "local", "other", "unknown:other", "peer:", "channel:"] {
        assert!(parse(encoded).is_err());
    }
}
