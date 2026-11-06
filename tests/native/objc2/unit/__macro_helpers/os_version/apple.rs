
    use super::*;

    use alloc::string::String;
    use std::process::Command;

    #[test]
    fn sysctl_same_as_in_plist() {
        if let Some(version) = version_from_sysctl() {
            assert_eq!(version, version_from_plist());
        }
    }

    #[test]
    fn read_version() {
        assert!(OSVersion::MIN < current_version(), "version cannot be min");
        assert!(current_version() < OSVersion::MAX, "version cannot be max");
    }

    #[test]
    #[cfg_attr(
        not(target_os = "macos"),
        ignore = "`sw_vers` is only available on macOS"
    )]
    fn compare_against_sw_vers() {
        let expected = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .unwrap()
            .stdout;
        let expected = String::from_utf8(expected).unwrap();
        let expected = OSVersion::from_str(expected.trim());

        let actual = current_version();
        assert_eq!(expected, actual);
    }

