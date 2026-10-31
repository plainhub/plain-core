use super::*;
fn profile() -> Profile {
    Profile {
        abi_order: vec!["arm64-v8a".into(), "armeabi-v7a".into()],
        density_dpi: 480,
        locale_tags: vec!["zh-CN".into(), "en-US".into()],
    }
}
fn choose(names: &[&str], profile: &Profile) -> Vec<String> {
    select(
        &names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>(),
        profile,
    )
    .into_iter()
    .map(str::to_owned)
    .collect()
}
#[test]
fn sai_and_bundletool_layouts_keep_device_configs() {
    assert_eq!(
        choose(
            &[
                "split_config.armeabi_v7a.apk",
                "split_config.arm64_v8a.apk",
                "split_config.xhdpi.apk",
                "split_config.xxhdpi.apk",
                "split_config.en.apk"
            ],
            &profile()
        ),
        [
            "split_config.arm64_v8a.apk",
            "split_config.xxhdpi.apk",
            "split_config.en.apk"
        ]
    );
    assert_eq!(
        choose(
            &[
                "splits/base-arm64_v8a.apk",
                "splits/base-x86.apk",
                "splits/base-hdpi.apk",
                "splits/base-xxhdpi.apk",
                "splits/base-en.apk",
                "splits/base-zh.apk"
            ],
            &profile()
        ),
        [
            "splits/base-arm64_v8a.apk",
            "splits/base-xxhdpi.apk",
            "splits/base-zh.apk"
        ]
    );
}
#[test]
fn feature_modules_keep_masters_and_matching_abis_without_duplicates() {
    assert_eq!(
        choose(
            &[
                "split_myfeature.apk",
                "split_myfeature.config.arm64_v8a.apk",
                "split_myfeature.config.x86.apk"
            ],
            &profile()
        ),
        [
            "split_myfeature.apk",
            "split_myfeature.config.arm64_v8a.apk"
        ]
    );
    assert_eq!(
        choose(
            &[
                "myfeature-master.apk",
                "myfeature-arm64_v8a.apk",
                "myfeature-en.apk"
            ],
            &profile()
        ),
        [
            "myfeature-master.apk",
            "myfeature-arm64_v8a.apk",
            "myfeature-en.apk"
        ]
    );
    assert!(
        choose(
            &["split_config.x86.apk", "split_config.x86_64.apk"],
            &profile()
        )
        .is_empty()
    );
    assert_eq!(
        choose(
            &["split_config.arm64-v8a.apk", "split_config.arm64-v8a.apk"],
            &profile()
        ),
        ["split_config.arm64-v8a.apk"]
    );
}
#[test]
fn language_selection_uses_exact_then_language_then_english() {
    let mut profile = profile();
    profile.locale_tags = vec!["pt-BR".into()];
    assert_eq!(
        choose(&["split_config.pt.apk", "split_config.pt-BR.apk"], &profile),
        ["split_config.pt-BR.apk"]
    );
    profile.locale_tags = vec!["zh-Hans-CN".into()];
    assert_eq!(
        choose(&["split_config.zh.apk", "split_config.en.apk"], &profile),
        ["split_config.zh.apk"]
    );
    profile.locale_tags = vec!["de-DE".into()];
    assert_eq!(
        choose(&["split_config.fr.apk", "split_config.en.apk"], &profile),
        ["split_config.en.apk"]
    );
    assert!(choose(&["split_config.fr.apk"], &profile).is_empty());
}
#[test]
fn nearest_density_ties_and_sidecars_match_existing_policy() {
    let mut profile = profile();
    profile.density_dpi = 213;
    assert_eq!(
        choose(
            &[
                "split_config.mdpi.apk",
                "split_config.hdpi.apk",
                "split_config.xhdpi.apk"
            ],
            &profile
        ),
        ["split_config.hdpi.apk"]
    );
    profile.density_dpi = 320;
    assert_eq!(
        choose(
            &["split_config.nodpi.apk", "split_config.xhdpi.apk"],
            &profile
        ),
        ["split_config.xhdpi.apk"]
    );
    assert!(sidecar("standalones/base-arm64_v8a.apk"));
    assert!(sidecar("instant/base-master.apk"));
    assert!(sidecar("apex/base.apk"));
    assert!(!sidecar("splits/base-master.apk"));
    assert!(!sidecar("base.apk"));
}
