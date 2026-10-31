use super::*;

#[test]
fn locale_preserves_script_and_region_for_country_names() {
    let locale = Locale::parse("zh-Hant-TW-u-nu-hanidec");
    assert_eq!(locale.language, "zh");
    assert_eq!(locale.script, "Hant");
    assert_eq!(locale.region, "TW");
    assert_eq!(
        locale.country_tags(),
        ["zh-Hant-TW", "zh-Hant", "zh-TW", "zh", "und"]
    );
}

#[test]
fn language_aliases_and_missing_locales_have_a_consistent_fallback() {
    for (tag, language) in [
        ("iw-IL", "he"),
        ("in-ID", "id"),
        ("ji", "yi"),
        ("EN-us", "en"),
    ] {
        assert_eq!(Locale::parse(tag).language, language);
    }
    for tag in ["", "und", "en_US", "invalidlocale", "x-private"] {
        assert_eq!(Locale::parse(tag).country_tags(), ["und"]);
    }
}
