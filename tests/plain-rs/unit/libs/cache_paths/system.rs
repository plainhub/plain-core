use super::*;

#[test]
fn xdg_cache_uses_absolute_configuration_or_the_home_default() {
    let home = std::env::temp_dir().join("cache-location-test/home");
    for value in [
        None,
        Some(OsStr::new("")),
        Some(OsStr::new("relative/cache")),
    ] {
        assert_eq!(
            base(Family::Xdg, Some(home.as_os_str()), value, None).unwrap(),
            home.join(".cache")
        );
    }
    assert_eq!(
        base(
            Family::Xdg,
            None,
            Some(
                std::env::temp_dir()
                    .join("cache-location-test/cache")
                    .as_os_str()
            ),
            None
        )
        .unwrap(),
        std::env::temp_dir().join("cache-location-test/cache")
    );
    assert!(base(Family::Xdg, None, None, None).is_err());
}

#[test]
fn apple_cache_uses_the_users_library_and_native_roots_are_scoped_to_one_data_directory() {
    let home = std::env::temp_dir().join("cache-location-test/home");
    let root = base(Family::Apple, Some(home.as_os_str()), None, None).unwrap();
    assert_eq!(root, home.join("Library/Caches"));
    let first = scoped(&root, &home.join("data"));
    assert!(first.starts_with(root.join("plain")));
    assert_eq!(first, scoped(&root, &home.join("other/../data/.")));
    assert_ne!(first, scoped(&root, &home.join("other-data")));
}
