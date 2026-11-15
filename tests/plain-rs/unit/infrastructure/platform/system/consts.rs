use super::*;

#[test]
fn linux_default_data_directory_is_fixed_for_all_users() {
    assert_eq!(
        default_data_directory("linux"),
        PathBuf::from("/var/lib/plainapp")
    );
}

#[test]
fn other_operating_systems_preserve_the_home_data_directory() {
    let expected = home_directory()
        .unwrap_or_else(std::env::temp_dir)
        .join(".plainapp/data");
    for os in ["macos", "windows"] {
        assert_eq!(default_data_directory(os), expected);
    }
}
