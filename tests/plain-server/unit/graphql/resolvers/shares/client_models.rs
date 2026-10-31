use super::*;

#[test]
fn shared_info_converts_guest_epoch_millis_to_main_instant_without_precision_loss() {
    for value in [None, Some(0), Some(1_893_456_000_123)] {
        let info = client::Info {
            name: "Share".into(),
            read_only: true,
            requires_password: false,
            expires_at: value,
            url_token: "resource".into(),
            entries: vec![],
        };
        let result = SharedLinkInfo::try_from(info.clone()).unwrap();
        assert_eq!(result.expires_at.map(|at| at.0.timestamp_millis()), value);
        let mut invalid = info;
        invalid.expires_at = Some(i64::MAX);
        assert!(SharedLinkInfo::try_from(invalid).is_err());
    }
}
