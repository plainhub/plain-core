use super::*;

#[test]
fn pmset_parse_charging() {
    let out =
        "Now drawing from 'AC Power'\n -InternalBattery-0 (id=123)	64%; charging; 2:11 remaining\n";
    assert_eq!(parse_pmset_battery(out), Some((64, true)));
}

#[test]
fn pmset_parse_discharging_is_not_charging() {
    let out = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=123)	87%; discharging; 3:28 remaining\n";
    assert_eq!(parse_pmset_battery(out), Some((87, false)));
}

#[test]
fn pmset_parse_full_while_plugged_is_not_charging() {
    let out =
        "Now drawing from 'AC Power'\n -InternalBattery-0 (id=123)	100%; charged; 0:00 remaining\n";
    assert_eq!(parse_pmset_battery(out), Some((100, false)));
    let out = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=123)	99%; finishing charge\n";
    assert_eq!(parse_pmset_battery(out), Some((99, false)));
}

#[test]
fn pmset_parse_desktop_without_battery_is_none() {
    assert_eq!(parse_pmset_battery("Now drawing from 'AC Power'\n"), None);
    assert_eq!(parse_pmset_battery(""), None);
}

#[test]
fn win_battery_maps_flags() {
    assert_eq!(win_battery_from(128, 80), None);
    assert_eq!(win_battery_from(1, 255), None);
    assert_eq!(win_battery_from(1, 101), None);
    assert_eq!(win_battery_from(8, 55), Some((55, true)));
    assert_eq!(win_battery_from(1, 55), Some((55, false)));
    assert_eq!(win_battery_from(0, 100), Some((100, false)));
}
