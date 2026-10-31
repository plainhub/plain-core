use super::*;
use serde_json::json;

#[test]
fn shared_card_input_preserves_large_sizes_expiry_and_peer_identity() {
    let input = SharedMessageCardInput {
        share_id: "share".into(),
        url_token: "token".into(),
        name: "Folder".into(),
        peer_info: SharedPeerAddressInput {
            id: "peer".into(),
            ip: "::1".into(),
            port: 8443,
        },
        item_count: Long(2),
        total_size: Long(9_000_000_000),
        expires_at: Some(Instant(
            chrono::DateTime::parse_from_rfc3339("2030-01-01T00:00:00.123Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        )),
    };
    let card: share_card::Card = input.try_into().unwrap();
    assert_eq!(card.peer_info.port, 8443);
    assert_eq!(card.total_size, 9_000_000_000);
    assert_eq!(card.expires_at.unwrap().timestamp_subsec_millis(), 123);
}

fn link(port: i32, page_url: Option<String>) -> SharedLinkInput {
    SharedLinkInput {
        host: "fixture.invalid".into(),
        port,
        shared_id: "share".into(),
        token: "fixture-token".into(),
        page_url,
    }
}
fn card(port: i32, expires_at: Option<Instant>) -> SharedMessageCardInput {
    SharedMessageCardInput {
        share_id: "share".into(),
        url_token: "resource".into(),
        peer_info: SharedPeerAddressInput {
            id: "peer".into(),
            ip: "::1".into(),
            port,
        },
        name: "Folder".into(),
        item_count: Long(2),
        total_size: Long(i64::MAX),
        expires_at,
    }
}

#[test]
fn owned_link_input_reuses_its_strings_and_preserves_optional_page_urls() {
    let input = link(
        8443,
        Some("https://fixture.invalid/s/share#fragment".into()),
    );
    let allocations = [
        input.host.as_ptr(),
        input.shared_id.0.as_ptr(),
        input.token.as_ptr(),
        input.page_url.as_ref().unwrap().as_ptr(),
    ];
    let output: client::Link = input.try_into().unwrap();
    assert_eq!(
        [
            output.host.as_ptr(),
            output.shared_id.as_ptr(),
            output.token.as_ptr(),
            output.page_url.as_ptr()
        ],
        allocations
    );
    assert_eq!(output.host, "fixture.invalid");
    assert_eq!(output.shared_id, "share");
    assert_eq!(output.token, "fixture-token");
    assert_eq!(output.page_url, "https://fixture.invalid/s/share#fragment");
    assert_eq!(
        client::Link::try_from(link(8443, None)).unwrap().page_url,
        ""
    );
    assert_eq!(
        client::Link::try_from(link(8443, Some(String::new())))
            .unwrap()
            .page_url,
        ""
    );
}

#[test]
fn typed_ports_keep_the_unsigned_range_and_endpoint_validation_boundary() {
    for value in [i32::MIN, -1, 65536, i32::MAX] {
        assert!(client::Link::try_from(link(value, None)).is_err());
        assert!(share_card::Card::try_from(card(value, None)).is_err());
    }
    for value in [0, 1, 65535] {
        let converted = client::Link::try_from(link(value, None)).unwrap();
        assert_eq!(i32::from(converted.port), value);
        assert_eq!(
            i32::from(
                share_card::Card::try_from(card(value, None))
                    .unwrap()
                    .peer_info
                    .port
            ),
            value
        );
    }
    assert!(
        client::Link::try_from(link(0, None))
            .unwrap()
            .url("/")
            .is_err()
    );
}

#[test]
fn typed_cards_match_the_previous_scalar_encoding_at_precision_and_integer_boundaries() {
    for expiry in [
        None,
        Some("2030-01-01T08:00:00.123456789+08:00"),
        Some("1969-12-31T23:59:59.999999999Z"),
        Some("2016-12-31T23:59:60.987654321Z"),
    ] {
        for size in [i64::MIN, -1, 0, 9_000_000_001, i64::MAX] {
            let expires = expiry.map(|value| {
                Instant(
                    chrono::DateTime::parse_from_rfc3339(value)
                        .unwrap()
                        .with_timezone(&chrono::Utc),
                )
            });
            let mut input = card(65535, expires);
            input.item_count = Long(size);
            input.total_size = Long(size);
            let legacy: share_card::Card = serde_json::from_value(json!({
                "shareId":input.share_id,"urlToken":input.url_token,"peerInfo":{"id":input.peer_info.id,"ip":input.peer_info.ip,"port":input.peer_info.port},
                "name":input.name,"itemCount":input.item_count,"totalSize":input.total_size,"expiresAt":input.expires_at,
            })).unwrap();
            assert_eq!(share_card::Card::try_from(input).unwrap(), legacy);
        }
    }
}

#[test]
fn owned_entries_keep_their_storage_and_full_width_size() {
    for size in [i64::MIN, -1, 0, i64::MAX] {
        let input = SharedEntryInput {
            name: "a +?#% file".into(),
            relative_path: "folder/a +?#% file".into(),
            is_dir: false,
            size: Long(size),
            mime_type: "text/plain".into(),
            has_thumb: true,
        };
        let allocations = [
            input.name.as_ptr(),
            input.relative_path.as_ptr(),
            input.mime_type.as_ptr(),
        ];
        let output: client::File = input.into();
        assert_eq!(
            [
                output.name.as_ptr(),
                output.relative_path.as_ptr(),
                output.mime_type.as_ptr()
            ],
            allocations
        );
        assert_eq!(output.name, "a +?#% file");
        assert_eq!(output.relative_path, "folder/a +?#% file");
        assert_eq!(output.mime_type, "text/plain");
        assert_eq!(output.size, size);
        assert!(!output.is_dir);
        assert!(output.has_thumb);
    }
}
