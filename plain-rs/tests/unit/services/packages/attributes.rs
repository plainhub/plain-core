use super::*;
#[test]
fn enums_include_existing_and_current_activity_values() {
    for (name, value, expected) in [
        ("screenOrientation", "-1", "unspecified"),
        ("screenOrientation", "1", "portrait"),
        ("launchMode", "4", "singleInstancePerTask"),
        ("installLocation", "2", "preferExternal"),
    ] {
        assert_eq!(display(name, value.into()), expected);
    }
    assert_eq!(display("launchMode", "99".into()), "99");
    assert_eq!(display("label", "42".into()), "42");
}
#[test]
fn config_flags_keep_all_known_flags_and_unknown_bits() {
    let output = display("configChanges", "0x2084".into());
    assert_eq!(output, "locale|orientation|layoutDirection");
    assert_eq!(
        display("configChanges", "0x10084".into()),
        "locale|orientation|0x10000"
    );
    assert_eq!(display("configChanges", "0".into()), "0");
}
#[test]
fn protection_and_window_flags_preserve_their_masks() {
    assert_eq!(
        display("protectionLevel", "0x32".into()),
        "signature|privileged|development"
    );
    assert_eq!(display("protectionLevel", "0".into()), "normal");
    assert_eq!(
        display("windowSoftInputMode", "0x215".into()),
        "stateAlwaysVisible|adjustResize|modeChanged"
    );
    assert_eq!(
        display("windowSoftInputMode", "0x411".into()),
        "stateUnchanged|adjustResize|0x400"
    );
}
