use super::*;

fn description() -> Description {
    Description {
        pattern: String::new().into(),
        lengths: Vec::new(),
    }
}

fn plan(general: &str, prefix: &str, transform: &str, lengths: &[usize]) -> Plan {
    Plan {
        region: "TEST".into(),
        country_code: 999,
        international_prefix: String::new().into(),
        national_prefix: prefix.to_owned().into(),
        transform: transform.into(),
        leading_digits: String::new().into(),
        general: general.to_owned().into(),
        lengths: lengths.into(),
        fixed_line: description(),
        mobile: description(),
        toll_free: description(),
        premium_rate: description(),
        shared_cost: description(),
        personal_number: description(),
        voip: description(),
        pager: description(),
        uan: description(),
        voicemail: description(),
    }
}

#[test]
fn every_bundled_pattern_compiles_including_unexercised_dialing_prefixes() {
    assert_eq!(PLANS.plans.len(), 254);
    assert_eq!(PLANS.decimal_zeroes.len(), 76);
    for plan in &PLANS.plans {
        for pattern in [
            &plan.general,
            &plan.leading_digits,
            &plan.national_prefix,
            &plan.international_prefix,
            &plan.fixed_line.pattern,
            &plan.mobile.pattern,
            &plan.toll_free.pattern,
            &plan.premium_rate.pattern,
            &plan.shared_cost.pattern,
            &plan.personal_number.pattern,
            &plan.voip.pattern,
            &plan.pager.pattern,
            &plan.uan.pattern,
            &plan.voicemail.pattern,
        ] {
            if !pattern.source.is_empty() {
                pattern.regex();
            }
        }
    }
}

#[test]
fn prefix_stripping_keeps_valid_originals_and_rejects_impossible_remaining_lengths() {
    for (general, lengths, original, expected) in [
        (r"^(?:0123)$", vec![4], "0123", "0123"),
        (r"^(?:123)$", vec![3], "0123", "123"),
        (r"^(?:\d+)$", vec![4, 6], "012345", "012345"),
        (r"^(?:\d+)$", vec![4, 6], "01234567", "1234567"),
    ] {
        let plan = plan(general, r"^(?:0)", "", &lengths);
        let mut number = original.to_owned();
        plan.strip_national_prefix(&mut number);
        assert_eq!(number, expected);
    }
}

#[test]
fn captured_prefix_transformations_require_the_participating_capture_groups() {
    for (prefix, transform, general, expected) in [
        (r"^(?:0(12))", "9${1}", r"^(?:91234)$", "91234"),
        (r"^(?:0()(12))", "9${2}", r"^(?:91234)$", "91234"),
        (r"^(?:0(?:(x))?(12))", "9${2}", r"^(?:34)$", "34"),
        (r"^(?:0(12)())", "9${1}", r"^(?:34)$", "34"),
    ] {
        let plan = plan(general, prefix, transform, &[2, 5]);
        let mut number = "01234".to_owned();
        plan.strip_national_prefix(&mut number);
        assert_eq!(number, expected);
    }
}

#[test]
fn empty_prefix_matches_and_invalid_transformations_preserve_the_owned_buffer() {
    for (prefix, transform) in [(r"^(?:0?)", ""), (r"^(?:0?(1234))", "9${1}")] {
        let plan = plan(r"^(?:0?1234)$", prefix, transform, &[4, 5]);
        let mut number = "1234".to_owned();
        let original = number.as_ptr();
        plan.strip_national_prefix(&mut number);
        assert_eq!(number, "1234");
        assert_eq!(number.as_ptr(), original);
    }
}
