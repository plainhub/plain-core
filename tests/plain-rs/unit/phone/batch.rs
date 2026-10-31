use super::*;
use crate::phone::Service;

#[tokio::test]
async fn batch_resolution_preserves_order_and_handles_invalid_numbers_without_a_host() {
    let runtime = Arc::new(Service::default());
    let rows = lookup_many(
        runtime.clone(),
        vec![
            "800-555-0100".into(),
            "not a number".into(),
            "800-555-0100".into(),
        ],
        LocaleFacts {
            region: "US".into(),
            locale: "en-US".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 3);
    let first = rows[0].as_ref().unwrap();
    assert_eq!(first.country, "US");
    assert_eq!(first.number_type, "TOLL_FREE");
    assert!(first.carrier.is_empty());
    assert!(rows[1].is_none());
    assert_eq!(rows[2].as_ref().unwrap().country, first.country);
    assert!(
        lookup_many(
            runtime,
            vec![],
            LocaleFacts {
                region: String::new(),
                locale: String::new(),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .is_empty()
    );
}

#[tokio::test]
async fn raw_network_region_takes_precedence_for_national_numbers() {
    let locale = serde_json::from_value(serde_json::json!({
        "region":"CN", "locale":"en-US", "networkRegion":"US", "simRegion":"CN"
    }))
    .unwrap();
    let rows = lookup_many(
        Arc::new(Service::default()),
        vec!["800-555-0100".into()],
        locale,
    )
    .await
    .unwrap();
    assert_eq!(rows[0].as_ref().unwrap().country, "US");
}
