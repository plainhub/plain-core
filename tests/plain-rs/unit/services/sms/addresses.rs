use super::*;

#[test]
fn selection_preserves_order_and_filters_own_numbers_with_unambiguous_suffixes() {
    let values = ["+1 (602) 555-0100", "+1 480 555 0101", "+1 623 555 0102"]
        .map(str::to_owned)
        .to_vec();
    assert_eq!(
        select(values, &["6025550100".into()]),
        ["+1 480 555 0101", "+1 623 555 0102"]
    );
    assert_eq!(
        select(vec!["+1 602 555 0100".into()], &["6025550100".into()]),
        ["+1 602 555 0100"]
    );
}
#[test]
fn normalized_duplicates_are_removed_without_collapsing_different_country_codes() {
    assert_eq!(
        select(
            vec!["Recipient@Local".into(), "recipient@local".into()],
            &[]
        ),
        ["Recipient@Local"]
    );
    let values: Vec<String> = vec!["+1 602 555 0100".into(), "+44 602 555 0100".into()];
    assert!(!matches(&values[0], "6025550100", &values));
    assert_eq!(select(values.clone(), &["6025550100".into()]), values);
    assert!(!matches("+44 20 1234 5678", "+1 20 1234 5678", &[]));
    assert!(matches("12-345", "12345", &[]));
}

#[test]
fn mms_address_projection_prefers_the_sender_then_the_first_available_candidate() {
    let values = vec![
        MmsAddress {
            value: "recipient".into(),
            is_sender: false,
        },
        MmsAddress {
            value: "sender".into(),
            is_sender: true,
        },
    ];
    assert_eq!(mms(&values), "sender");
    assert_eq!(mms(&values[..1]), "recipient");
    assert_eq!(mms(&[]), "");
}

#[test]
fn mms_ignores_provider_placeholder_addresses_before_sender_selection() {
    let values = vec![
        MmsAddress {
            value: "INSERT-ADDRESS-TOKEN".into(),
            is_sender: true,
        },
        MmsAddress {
            value: String::new(),
            is_sender: true,
        },
        MmsAddress {
            value: "recipient".into(),
            is_sender: false,
        },
    ];
    assert_eq!(mms(&values), "recipient");
    assert_eq!(mms(&values[..2]), "");
}
