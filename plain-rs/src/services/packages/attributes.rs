const ORIENTATION: &[(u32, &str)] = &[
    (0x00000000, "landscape"),
    (0x00000001, "portrait"),
    (0x00000002, "user"),
    (0x00000003, "behind"),
    (0x00000004, "sensor"),
    (0x00000005, "nosensor"),
    (0x00000006, "sensorLandscape"),
    (0x00000007, "sensorPortrait"),
    (0x00000008, "reverseLandscape"),
    (0x00000009, "reversePortrait"),
    (0x0000000a, "fullSensor"),
    (0x0000000b, "userLandscape"),
    (0x0000000c, "userPortrait"),
    (0x0000000d, "fullUser"),
    (0x0000000e, "locked"),
    (0xffffffff, "unspecified"),
];
const LAUNCH_MODE: &[(u32, &str)] = &[
    (0x00000000, "standard"),
    (0x00000001, "singleTop"),
    (0x00000002, "singleTask"),
    (0x00000003, "singleInstance"),
    (0x00000004, "singleInstancePerTask"),
];
const INSTALL_LOCATION: &[(u32, &str)] = &[
    (0x00000000, "auto"),
    (0x00000001, "internalOnly"),
    (0x00000002, "preferExternal"),
];
const CONFIG_CHANGES: &[(u32, &str)] = &[
    (0x00000001, "mcc"),
    (0x00000002, "mnc"),
    (0x00000004, "locale"),
    (0x00000008, "touchscreen"),
    (0x00000010, "keyboard"),
    (0x00000020, "keyboardHidden"),
    (0x00000040, "navigation"),
    (0x00000080, "orientation"),
    (0x00000100, "screenLayout"),
    (0x00000200, "uiMode"),
    (0x00000400, "screenSize"),
    (0x00000800, "smallestScreenSize"),
    (0x00001000, "density"),
    (0x00002000, "layoutDirection"),
    (0x00004000, "colorMode"),
    (0x00008000, "grammaticalGender"),
    (0x08000000, "resourcesUnused"),
    (0x10000000, "fontWeightAdjustment"),
    (0x40000000, "fontScale"),
    (0x80000000, "assetsPaths"),
];
const PROTECTION: &[(u32, &str)] = &[
    (0x00000010, "privileged"),
    (0x00000020, "development"),
    (0x00000040, "appop"),
    (0x00000080, "pre23"),
    (0x00000100, "installer"),
    (0x00000200, "verifier"),
    (0x00000400, "preinstalled"),
    (0x00000800, "setup"),
    (0x00001000, "instant"),
    (0x00002000, "runtime"),
    (0x00004000, "oem"),
    (0x00008000, "vendorPrivileged"),
    (0x00010000, "textClassifier"),
    (0x00080000, "configurator"),
    (0x00100000, "incidentReportApprover"),
    (0x00200000, "appPredictor"),
    (0x00400000, "module"),
    (0x00800000, "companion"),
    (0x01000000, "retailDemo"),
    (0x02000000, "recents"),
    (0x04000000, "role"),
    (0x08000000, "knownSigner"),
];

fn flags(mut value: u32, table: &[(u32, &str)]) -> Vec<String> {
    let mut result = Vec::new();
    for (bit, name) in table {
        if *bit != 0 && value & *bit == *bit {
            result.push((*name).to_owned());
            value &= !*bit;
        }
    }
    if value != 0 {
        result.push(format!("0x{value:x}"));
    }
    result
}
pub(super) fn display(name: &str, value: String) -> String {
    let number = if let Some(hex) = value.strip_prefix("0x") {
        u32::from_str_radix(hex, 16).ok()
    } else {
        value.parse::<i64>().ok().and_then(|n| {
            i32::try_from(n)
                .map(|n| n as u32)
                .ok()
                .or_else(|| u32::try_from(n).ok())
        })
    };
    let Some(number) = number else { return value };
    let enumeration = match name {
        "screenOrientation" => Some(ORIENTATION),
        "launchMode" => Some(LAUNCH_MODE),
        "installLocation" => Some(INSTALL_LOCATION),
        _ => None,
    };
    if let Some(table) = enumeration {
        return table
            .iter()
            .find(|(key, _)| *key == number)
            .map(|(_, name)| (*name).to_owned())
            .unwrap_or(value);
    }
    let parts = match name {
        "configChanges" => flags(number, CONFIG_CHANGES),
        "protectionLevel" => {
            let level = number & 0xf;
            let base = match level {
                0 => "normal".into(),
                1 => "dangerous".into(),
                2 => "signature".into(),
                3 => "signatureOrSystem".into(),
                4 => "internal".into(),
                _ => format!("0x{level:x}"),
            };
            let mut parts = vec![base];
            parts.extend(flags(number & !0xf, PROTECTION));
            parts
        }
        "windowSoftInputMode" => {
            let mut parts = Vec::new();
            match number & 0xf {
                0 => {}
                1 => parts.push("stateUnchanged".into()),
                2 => parts.push("stateHidden".into()),
                3 => parts.push("stateAlwaysHidden".into()),
                4 => parts.push("stateVisible".into()),
                5 => parts.push("stateAlwaysVisible".into()),
                n => parts.push(format!("0x{n:x}")),
            }
            match number & 0xf0 {
                0 => {}
                0x10 => parts.push("adjustResize".into()),
                0x20 => parts.push("adjustPan".into()),
                0x30 => parts.push("adjustNothing".into()),
                n => parts.push(format!("0x{n:x}")),
            }
            parts.extend(flags(
                number & !0xff,
                &[(0x100, "isForwardNavigation"), (0x200, "modeChanged")],
            ));
            parts
        }
        _ => return value,
    };
    if parts.is_empty() {
        value
    } else {
        parts.join("|")
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/packages/attributes.rs"]
pub(crate) mod tests;
