use super::*;

fn dev(name: &str, kind: &str, rm: Option<serde_json::Value>) -> LsblkDevice {
    LsblkDevice {
        name: name.into(),
        path: format!("/dev/{name}"),
        kind: kind.into(),
        size: Some(1_000_000_000),
        rm,
        model: Some("Test Model".into()),
        ..Default::default()
    }
}

#[test]
fn only_top_level_disks_survive() {
    let devs = vec![
        dev("sda", "disk", Some(serde_json::json!(false))),
        dev("sda1", "part", None),
        dev("zram0", "disk", None),
        dev("loop0", "disk", None),
        dev("ram0", "disk", None),
        dev("sr0", "rom", None),
    ];
    let disks = disks_from_lsblk(&devs);
    assert_eq!(disks.len(), 1);
    assert_eq!(disks[0].name, "sda");
    assert_eq!(disks[0].path, "/dev/sda");
    assert_eq!(disks[0].size_bytes, 1_000_000_000);
    assert!(!disks[0].removable);
}

#[test]
fn removable_from_rm_bool() {
    let disks = disks_from_lsblk(&[dev("sdb", "disk", Some(serde_json::json!(true)))]);
    assert!(disks[0].removable);
}

#[test]
fn removable_boolish_string() {
    let disks = disks_from_lsblk(&[dev("sdc", "disk", Some(serde_json::json!("1")))]);
    assert!(disks[0].removable);
}

#[test]
fn path_defaults_to_dev_slash_name() {
    let d = LsblkDevice {
        name: "sdd".into(),
        path: "".into(),
        kind: "disk".into(),
        rm: None,
        ..Default::default()
    };
    let disks = disks_from_lsblk(&[d]);
    assert_eq!(disks[0].path, "/dev/sdd");
}

#[test]
fn id_uses_disk_prefix_when_no_by_id() {
    let disks = disks_from_lsblk(&[dev("definitely-not-real-abc", "disk", None)]);
    assert_eq!(disks[0].id, "disk:definitely-not-real-abc");
}

#[test]
fn lsblk_model_used_when_sysfs_missing() {
    let disks = disks_from_lsblk(&[dev("definitely-not-real-abc", "disk", None)]);
    assert_eq!(disks[0].model.as_deref(), Some("Test Model"));
}

#[test]
fn empty_model_stays_none() {
    let d = LsblkDevice {
        name: "definitely-not-real-abc".into(),
        path: "/dev/definitely-not-real-abc".into(),
        kind: "disk".into(),
        model: Some("   ".into()),
        rm: None,
        ..Default::default()
    };
    let disks = disks_from_lsblk(&[d]);
    assert_eq!(disks[0].model, None);
}

#[test]
fn a_successful_empty_read_stays_empty() {
    let disks = read_with(|columns| {
        assert_eq!(columns, &["NAME", "PATH", "TYPE", "MODEL", "SIZE", "RM"]);
        Ok(Vec::new())
    })
    .unwrap();
    assert!(disks.is_empty());
}

#[test]
fn a_failed_read_is_reported_without_empty_success() {
    let error = read_with(|_| Err(anyhow::anyhow!("lsblk unavailable"))).unwrap_err();
    assert_eq!(error, "lsblk unavailable");
}

#[test]
fn a_disk_read_runs_one_collector_and_filters_its_rows() {
    let calls = std::cell::Cell::new(0);
    let disks = read_with(|_| {
        calls.set(calls.get() + 1);
        Ok(vec![
            dev(
                "definitely-not-real-disk",
                "disk",
                Some(serde_json::json!(true)),
            ),
            dev("definitely-not-real-part", "part", None),
        ])
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(disks.len(), 1);
    assert_eq!(disks[0].id, "disk:definitely-not-real-disk");
    assert_eq!(disks[0].path, "/dev/definitely-not-real-disk");
    assert_eq!(disks[0].model.as_deref(), Some("Test Model"));
    assert!(disks[0].removable);
}
