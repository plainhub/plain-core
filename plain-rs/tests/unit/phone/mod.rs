use super::*;
use flate2::read::GzDecoder;
use std::io::{BufRead, BufReader};
use std::sync::atomic::Ordering;

pub(super) fn example(region: &str, kind: NumberType) -> String {
    let bytes = include_bytes!("../../../testdata/phone/reference.jsonl.gz");
    BufReader::new(GzDecoder::new(bytes.as_slice()))
        .lines()
        .skip(1)
        .find_map(|line| {
            let case: Reference = serde_json::from_str(&line.unwrap()).unwrap();
            (case.region == region
                && case
                    .expected
                    .as_ref()
                    .is_some_and(|value| value.number_type == type_name(kind))
                && case.number.starts_with('+')
                && case.number[1..].bytes().all(|byte| byte.is_ascii_digit()))
            .then_some(case.number)
        })
        .unwrap()
}

#[derive(serde::Deserialize)]
struct Reference {
    region: String,
    number: String,
    locale: String,
    expected: Option<Metadata>,
}

#[test]
fn region_selection_uses_network_then_sim_then_locale_without_platform_rules() {
    assert_eq!(default_region("CN", "GB", "US"), "CN");
    assert_eq!(default_region(" ", "GB", "US"), "GB");
    assert_eq!(default_region("", "", "US"), "US");
    assert_eq!(default_region("", "", ""), "");
}

#[test]
fn phone_results_match_independent_google_examples_in_every_exported_format_and_locale() {
    let bytes = include_bytes!("../../../testdata/phone/reference.jsonl.gz");
    let mut lines = BufReader::new(GzDecoder::new(bytes.as_slice())).lines();
    let header: serde_json::Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(
        header["reference"]["source"],
        "google-libphonenumber-public-examples"
    );
    assert_eq!(header["reference"]["versions"]["libphonenumber"], "9.0.41");
    assert_eq!(header["reference"]["versions"]["geocoder"], "3.41");
    assert_eq!(header["reference"]["versions"]["carrier"], "2.41");
    let service = Service::default();
    let mut count = 0;
    let mut failures = Vec::new();
    let mut failed_count = 0;
    for (index, line) in lines.enumerate() {
        let case: Reference = serde_json::from_str(&line.unwrap()).unwrap();
        let actual = service.lookup(&case.number, &case.region, &case.locale);
        if actual != case.expected {
            failed_count += 1;
            if failures.len() < 20 {
                failures.push(format!(
                    "case {}, region {}, locale {}: actual {:?}, expected {:?}",
                    index + 1,
                    case.region,
                    case.locale,
                    actual,
                    case.expected
                ));
            }
        }
        count += 1;
    }
    assert_eq!(count, 81_324);
    assert_eq!(failed_count, 0, "{}", failures.join("\n"));
}

#[test]
fn repeated_valid_and_invalid_numbers_reuse_results_but_locale_and_region_do_not() {
    let service = Service::default();
    let number = example("CN", NumberType::Mobile);
    let first = service.lookup(&number, "CN", "zh-CN").unwrap();
    assert_eq!(first.country, "CN");
    assert_eq!(first.number_type, "MOBILE");
    assert!(!first.carrier.is_empty());
    assert_eq!(service.lookup(&number, "CN", "zh-CN"), Some(first));
    assert_eq!(service.resolutions.load(Ordering::Relaxed), 1);
    service.lookup(&number, "CN", "en").unwrap();
    service.lookup(&number, "US", "en").unwrap();
    assert_eq!(service.resolutions.load(Ordering::Relaxed), 3);
    for _ in 0..3 {
        assert!(service.lookup("not a number", "US", "en").is_none());
    }
    assert_eq!(service.resolutions.load(Ordering::Relaxed), 4);
}

#[test]
fn absent_region_never_guesses_national_numbers_and_explicit_country_codes_still_work() {
    let service = Service::default();
    let international = example("CN", NumberType::Mobile);
    let national = number::parse(&international, "").unwrap().national;
    assert!(service.lookup(&national, "", "en").is_none());
    assert_eq!(
        service.lookup(&international, "", "en").unwrap().country,
        "CN"
    );
    assert_eq!(
        service.lookup(&international, "us", "en").unwrap().country,
        "CN"
    );
    assert_eq!(service.lookup(&national, "CN", "en").unwrap().country, "CN");
}

#[test]
fn carrier_is_not_attached_to_toll_free_or_fixed_line_numbers() {
    let service = Service::default();
    for kind in [NumberType::TollFree, NumberType::FixedLine] {
        let number = example("GB", kind);
        let facts = service.lookup(&number, "GB", "en").unwrap();
        assert_eq!(facts.number_type, type_name(kind));
        assert!(facts.carrier.is_empty());
    }
}

#[test]
fn result_cache_evicts_old_entries_and_never_keeps_oversized_inputs() {
    let service = Service::default();
    for index in 0..1025 {
        service.lookup(&format!("invalid-{index}"), "US", "en");
    }
    assert_eq!(service.cache.lock().unwrap().len(), 1024);
    let count = service.resolutions.load(Ordering::Relaxed);
    service.lookup("invalid-0", "US", "en");
    assert_eq!(service.resolutions.load(Ordering::Relaxed), count + 1);
    service.lookup(&"x".repeat(4097), "US", "en");
    assert!(
        service
            .cache
            .lock()
            .unwrap()
            .iter()
            .all(|(key, _)| key.len() <= 4096)
    );
}
