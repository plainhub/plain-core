use super::*;

#[test]
fn dialing_prefixes_global_contexts_and_national_numbers_preserve_country_and_type() {
    let international = crate::phone::tests::example("CN", NumberType::Mobile);
    let canonical = parse(&international, "").unwrap();
    for (input, region) in [
        (format!("011{}", &international[1..]), "US"),
        (format!("00{}", &international[1..]), "GB"),
        (format!("+00{}", &international[1..]), "GB"),
        (
            format!(
                "tel:{};phone-context=+{}",
                canonical.national, canonical.country_code
            ),
            "",
        ),
        (canonical.national.clone(), "CN"),
    ] {
        let parsed = parse(&input, region).unwrap();
        assert_eq!(parsed.country_code, canonical.country_code);
        assert_eq!(parsed.national, canonical.national);
        assert_eq!(parsed.kind, canonical.kind);
    }
    assert!(parse(&canonical.national, "").is_none());
    assert!(parse(&format!("+0{}", &international[1..]), "CN").is_none());
    assert!(
        parse(
            &format!("{}!{}", &international[..4], &international[4..]),
            "CN"
        )
        .is_none()
    );
}

#[test]
fn national_trunk_prefixes_and_significant_italian_zeroes_are_kept_distinct() {
    let international = crate::phone::tests::example("GB", NumberType::FixedLine);
    let canonical = parse(&international, "").unwrap();
    let national = parse(&format!("0{}", canonical.national), "GB").unwrap();
    assert_eq!(national.national, canonical.national);
    let international = crate::phone::tests::example("IT", NumberType::FixedLine);
    let canonical = parse(&international, "").unwrap();
    assert!(canonical.national.starts_with('0'));
    assert_eq!(
        parse(&canonical.national, "IT").unwrap().national,
        canonical.national
    );
}

#[test]
fn absent_national_prefix_keeps_the_owned_number_buffer() {
    let international = crate::phone::tests::example("IT", NumberType::FixedLine);
    let mut number = parse(&international, "").unwrap().national;
    let original = number.as_ptr();
    let plan = &PLANS.plans[PLANS.regions["IT"]];
    plan.strip_national_prefix(&mut number);
    assert_eq!(number.as_ptr(), original);
}

#[derive(serde::Deserialize)]
struct CorpusHeader {
    reference: CorpusReference,
}

#[derive(serde::Deserialize)]
struct CorpusReference {
    source: String,
    normalization: String,
    version: String,
    case_count: usize,
    jar_sha256: String,
    examples_sha256: String,
}

#[derive(serde::Deserialize, Debug, PartialEq, Eq)]
struct Facts {
    country_code: u16,
    national: String,
    kind: String,
    main_region: String,
    valid_regions: Vec<String>,
}

#[derive(serde::Deserialize)]
struct Case {
    region: String,
    number: String,
    expected: Option<Facts>,
}

#[test]
fn parser_matches_the_independent_google_syntax_and_region_replay() {
    use flate2::read::GzDecoder;
    use sha2::{Digest, Sha256};
    use std::io::{BufRead, BufReader};
    let corpus = include_bytes!("../../../testdata/phone/parser-reference.jsonl.gz");
    let mut lines = BufReader::new(GzDecoder::new(corpus.as_slice())).lines();
    let header: CorpusHeader = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(
        header.reference.source,
        "google-libphonenumber-public-examples-and-syntax-variants"
    );
    assert_eq!(header.reference.version, "9.0.41");
    assert_eq!(header.reference.normalization, "unicode-decimal-codepoints");
    assert_eq!(header.reference.case_count, 45_631);
    assert_eq!(
        header.reference.jar_sha256,
        "c9efe6739636308930ecd2d3f282173e7d98a7c07592dbca466c02df8494f4a8"
    );
    assert_eq!(
        header.reference.examples_sha256,
        crate::utils::hex::bytes_to_hex(&Sha256::digest(include_bytes!(
            "../../../testdata/phone/reference.jsonl.gz"
        )))
    );
    let mut failures = Vec::new();
    let mut failed = 0;
    let mut count = 0;
    for line in lines {
        let case: Case = serde_json::from_str(&line.unwrap()).unwrap();
        let actual = parse(&case.number, &case.region).map(|number| {
            let mut valid_regions = regions(number.country_code)
                .filter(|plan| plan.kind(&number.national).is_some())
                .map(|plan| plan.region.clone())
                .collect::<Vec<_>>();
            valid_regions.sort();
            Facts {
                main_region: main_region(&number).to_owned(),
                country_code: number.country_code,
                national: number.national,
                kind: crate::phone::type_name(number.kind).to_owned(),
                valid_regions,
            }
        });
        if actual != case.expected {
            failed += 1;
            if failures.len() < 20 {
                failures.push(format!(
                    "case {count}, region {}, input {:?}: actual {actual:?}, expected {:?}",
                    case.region, case.number, case.expected
                ));
            }
        }
        count += 1;
    }
    assert_eq!(count, 45_631);
    assert_eq!(failed, 0, "{}", failures.join("\n"));
}
