use super::*;

#[test]
fn sysfs_wireless_and_cellular_facts_override_generic_ethernet_types() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("test0");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("type"), "1\n").unwrap();
    assert_eq!(read_type(root.path(), "test0"), InterfaceType::Ethernet);
    std::fs::write(path.join("uevent"), "DEVTYPE=wlan\n").unwrap();
    assert_eq!(
        read_type(root.path(), "test0"),
        InterfaceType::Wireless80211
    );
    std::fs::write(path.join("uevent"), "DEVTYPE=wwan\n").unwrap();
    assert_eq!(read_type(root.path(), "test0"), InterfaceType::Cellular);
    std::fs::remove_file(path.join("uevent")).unwrap();
    std::fs::create_dir(path.join("wireless")).unwrap();
    assert_eq!(
        read_type(root.path(), "test0"),
        InterfaceType::Wireless80211
    );
}

#[test]
fn missing_sysfs_keeps_known_mobile_interface_conventions_and_unknown_devices() {
    let root = tempfile::tempdir().unwrap();
    for name in ["wwan0", "wwp0s1", "rmnet_data0", "ccmni0"] {
        assert_eq!(read_type(root.path(), name), InterfaceType::Cellular);
    }
    assert_eq!(read_type(root.path(), "unknown0"), InterfaceType::Unknown);
    assert_eq!(
        interface_type("plain-test-missing-interface"),
        InterfaceType::Unknown
    );
}

#[test]
fn sysfs_types_keep_ppp_tunnels_and_unknown_values_distinct() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("test0");
    std::fs::create_dir(&path).unwrap();
    for (value, expected) in [
        ("512", InterfaceType::Ppp),
        ("768", InterfaceType::Tunnel),
        ("776", InterfaceType::Tunnel),
        ("invalid", InterfaceType::Unknown),
    ] {
        std::fs::write(path.join("type"), value).unwrap();
        assert_eq!(read_type(root.path(), "test0"), expected);
    }
    std::fs::create_dir(path.join("phy80211")).unwrap();
    assert_eq!(
        read_type(root.path(), "test0"),
        InterfaceType::Wireless80211
    );
}
