use super::*;
use serde_json::json;

fn source() -> Source {
    serde_json::from_value(
        json!({"url":"https://example.com/releases/latest", "assets":[
            {"requiredAbis":["arm64-v8a","x86_64"],"nameContains":"Recommended"},
            {"nameSuffix":"-Old-32bit.apk"}
        ]}),
    )
    .unwrap()
}
fn release() -> Release {
    serde_json::from_value(json!({"tag_name":"v3.4.2", "body":"Changes", "created_at":"created", "assets":[
        {"name":"Plain-Old-32bit.apk","size":32,"browser_download_url":"https://example.com/old.apk"},
        {"name":"Plain-Recommended.apk","size":5_000_000_000u64,"browser_download_url":"https://example.com/recommended.apk"}
    ]})).unwrap()
}
#[test]
fn update_policy_selects_configured_asset_and_preserves_user_and_download_state() {
    for (abi, expected) in [
        ("arm64-v8a", 5_000_000_000u64),
        ("x86_64", 5_000_000_000),
        ("armeabi-v7a", 32),
    ] {
        let mut info = UpdateInfo {
            downloaded_apk_path: "downloaded".into(),
            ..Default::default()
        };
        assert!(apply_release(
            &mut info,
            "3.3.25",
            "",
            &[abi.into()],
            &source(),
            &release()
        ));
        assert_eq!(info.new_version, "3.4.2");
        assert_eq!(info.size, expected);
        assert_eq!(info.publish_date, "created");
        assert_eq!(info.log, "Changes");
        assert_eq!(info.downloaded_apk_path, "downloaded");
    }
}
#[test]
fn skipped_current_and_older_versions_do_not_overwrite_existing_offer() {
    for (current, skip) in [
        ("3.4.2", ""),
        ("4.0.0", ""),
        ("3.3.25", "3.4.2"),
        ("3.3.25", "4.0.0"),
    ] {
        let mut info = UpdateInfo {
            new_version: "existing".into(),
            ..Default::default()
        };
        let before = info.clone();
        assert!(!apply_release(
            &mut info,
            current,
            skip,
            &[],
            &source(),
            &release()
        ));
        assert_eq!(info, before);
    }
}
#[test]
fn missing_asset_retains_release_details_without_silently_selecting_another_architecture() {
    let mut release = release();
    release.assets.remove(1);
    release.published_at = "published".into();
    let mut info = UpdateInfo::default();
    assert!(apply_release(
        &mut info,
        "3.3.25",
        "",
        &["arm64-v8a".into()],
        &source(),
        &release
    ));
    assert_eq!(info.size, 0);
    assert!(info.download_url.is_empty());
    assert_eq!(info.publish_date, "published");
}
#[test]
fn automatic_checks_observe_user_setting_day_boundary_and_clock_rollback() {
    let info = UpdateInfo {
        check_update_time: 100,
        ..Default::default()
    };
    assert!(!due(&info, true, 100 + 86_400_000));
    assert!(due(&info, true, 101 + 86_400_000));
    assert!(!due(&info, true, 0));
    assert!(!due(&info, false, u64::MAX));
}
#[test]
fn availability_uses_the_same_three_component_comparison_as_the_app_baseline() {
    let info = UpdateInfo {
        new_version: "3.4.2".into(),
        ..Default::default()
    };
    assert!(available(&info, "3.4.1", ""));
    assert!(!available(&info, "3.4.1", "3.4.2"));
    assert_eq!(Version::parse("3.4.invalid"), Version::parse("3.4.0"));
    assert_eq!(Version::parse("3.4.2.extra"), Version::parse("3.4.2"));
    assert!(serde_json::from_str::<Release>(r#"{"body":"missing tag"}"#).is_err());
}

#[test]
fn release_versions_normalize_distribution_prefixes_and_metadata() {
    for version in [
        "v1.2.3",
        "PlainApp 1.2.3",
        "desktop-v1.2.3",
        "1.2.3-beta.1",
        "1.2.3+abc",
        "  v1.2.3  ",
    ] {
        assert_eq!(normalized_version(version), "1.2.3");
        assert!(available_version("1.2.4", version, ""));
        assert!(!available_version(version, "1.2.4", ""));
    }
    assert_eq!(normalized_version("dev"), "dev");
    assert!(!available_version("any-later-tag", "dev", ""));
}

#[test]
fn release_metadata_uses_source_page_and_creation_time_only_when_missing() {
    let mut source = source();
    source.page_url = "https://example.com/releases/latest".into();
    let mut release = release();
    release.name = "Release name".into();
    let info = release_info(&release, &source).unwrap();
    assert_eq!(info.version, "3.4.2");
    assert_eq!(info.name, "Release name");
    assert_eq!(info.url, source.page_url);
    assert_eq!(info.published_at, "created");
    release.html_url = "https://example.com/releases/v3.4.2".into();
    release.published_at = "published".into();
    let info = release_info(&release, &source).unwrap();
    assert_eq!(info.url, release.html_url);
    assert_eq!(info.published_at, "published");
    release.tag_name = "invalid".into();
    assert!(release_info(&release, &source).is_err());
}
