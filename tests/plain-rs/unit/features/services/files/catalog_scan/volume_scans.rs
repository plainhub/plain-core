use super::*;
use crate::infrastructure::db::FileIndexMode;

fn fixture() -> (Db, Scope) {
    let db = Db::open(Path::new(":memory:")).unwrap();
    db.with_conn(|conn| conn.execute_batch("INSERT INTO volumes(id,identity,path,is_available,index_mode,included_dirs) VALUES('outer','outer','/fixture',1,'BLOCKLIST','[]'),('inner','inner','/fixture/disk',1,'ALLOWLIST','[\"photos\",\"docs\"]');")).unwrap();
    let volumes = db.volumes().unwrap();
    let scope = Scope {
        directories: source_directories(&volumes),
        exclusions: Vec::new(),
        defaults: Default::default(),
        volumes,
    };
    (db, scope)
}

#[test]
fn nested_volumes_keep_independent_counts_and_terminal_results() {
    let (db, scope) = fixture();
    assert!(
        scope
            .volumes
            .iter()
            .all(|volume| volume.last_scan_at.is_none())
    );
    let mut scans = VolumeScans::new(Some(&scope), &scope.directories, false);
    for file in [
        "/fixture/a.txt",
        "/fixture/disk/photos/b.jpg",
        "/fixture/disk/docs/c.txt",
    ] {
        scans.discovered_file(Path::new(file)).unwrap();
    }
    for (id, path) in [
        ("outer", "/fixture"),
        ("inner", "/fixture/disk/photos"),
        ("inner", "/fixture/disk/docs"),
    ] {
        scans.discovered_root(id, Path::new(path));
    }
    scans
        .processed(["/fixture/a.txt", "/fixture/disk/photos/b.jpg"])
        .unwrap();
    scans.failed(Path::new("/fixture/disk/docs/c.txt"));
    for (id, path) in [
        ("outer", "/fixture"),
        ("inner", "/fixture/disk/photos"),
        ("inner", "/fixture/disk/docs"),
    ] {
        scans.completed_root(id, Path::new(path));
    }
    scans.persist(&db, false).unwrap();
    let volumes = db.volumes().unwrap();
    let outer = volumes.iter().find(|volume| volume.id == "outer").unwrap();
    let inner = volumes.iter().find(|volume| volume.id == "inner").unwrap();
    assert_eq!(
        (
            outer.last_scan_status,
            outer.last_scan_count,
            outer.last_scan_total
        ),
        (Some(FileIndexScanStatus::Completed), Some(1), Some(1))
    );
    assert_eq!(
        (
            inner.last_scan_status,
            inner.last_scan_count,
            inner.last_scan_total
        ),
        (Some(FileIndexScanStatus::Failed), Some(1), Some(2))
    );
    assert!(outer.last_scan_at.is_some());
    assert!(scope.matches_volumes(&volumes));
}

#[test]
fn canceled_discovery_has_unknown_total_and_empty_completed_scan_has_zero() {
    let (db, scope) = fixture();
    let mut scans = VolumeScans::new(Some(&scope), &scope.directories, false);
    scans
        .discovered_file(Path::new("/fixture/one.txt"))
        .unwrap();
    scans.processed(["/fixture/one.txt"]).unwrap();
    scans.persist(&db, true).unwrap();
    let outer = db
        .volumes()
        .unwrap()
        .into_iter()
        .find(|volume| volume.id == "outer")
        .unwrap();
    assert_eq!(outer.last_scan_status, Some(FileIndexScanStatus::Canceled));
    assert_eq!(outer.last_scan_count, Some(1));
    assert_eq!(outer.last_scan_total, None);
    let mut scans = VolumeScans::new(Some(&scope), &scope.directories, false);
    for (id, path) in [
        ("outer", "/fixture"),
        ("inner", "/fixture/disk/photos"),
        ("inner", "/fixture/disk/docs"),
    ] {
        scans.discovered_root(id, Path::new(path));
    }
    for (id, path) in [
        ("outer", "/fixture"),
        ("inner", "/fixture/disk/photos"),
        ("inner", "/fixture/disk/docs"),
    ] {
        scans.completed_root(id, Path::new(path));
    }
    scans.persist(&db, false).unwrap();
    for volume in db.volumes().unwrap() {
        assert_eq!(
            volume.last_scan_status,
            Some(FileIndexScanStatus::Completed)
        );
        assert_eq!(volume.last_scan_count, Some(0));
        assert_eq!(volume.last_scan_total, Some(0));
    }
}

#[test]
fn background_and_partial_scans_preserve_history_and_configuration_changes_still_invalidate_scope()
{
    let (db, scope) = fixture();
    db.record_volume_scan("inner", FileIndexScanStatus::Completed, 7, Some(7))
        .unwrap();
    let before = db.volumes().unwrap();
    VolumeScans::new(Some(&scope), &scope.directories, true)
        .persist(&db, false)
        .unwrap();
    VolumeScans::new(Some(&scope), &["/fixture/disk/photos".into()], false)
        .persist(&db, false)
        .unwrap();
    assert_eq!(db.volumes().unwrap(), before);
    assert!(scope.matches_volumes(&before));
    let mut changed = before;
    changed[0].index_mode = FileIndexMode::Disabled;
    assert!(!scope.matches_volumes(&changed));
}

#[test]
fn scan_status_roundtrips_and_rejects_unknown_storage_and_wire_values() {
    let (db, _) = fixture();
    for (status, wire) in [
        (FileIndexScanStatus::Completed, "COMPLETED"),
        (FileIndexScanStatus::Canceled, "CANCELED"),
        (FileIndexScanStatus::Failed, "FAILED"),
    ] {
        db.record_volume_scan("outer", status, 1, Some(2)).unwrap();
        assert_eq!(
            db.volumes()
                .unwrap()
                .into_iter()
                .find(|volume| volume.id == "outer")
                .unwrap()
                .last_scan_status,
            Some(status)
        );
        assert_eq!(plain_libs::serde_json::to_value(status).unwrap(), wire);
        assert_eq!(
            plain_libs::serde_json::from_str::<FileIndexScanStatus>(&format!("\"{wire}\""))
                .unwrap(),
            status
        );
    }
    assert!(plain_libs::serde_json::from_str::<FileIndexScanStatus>("\"RUNNING\"").is_err());
    assert!(
        db.with_conn(|conn| conn.query_row("SELECT 'RUNNING'", [], |row| row
            .get::<_, FileIndexScanStatus>(0)))
            .is_err()
    );
    assert!(
        db.with_conn(|conn| conn.execute(
            "UPDATE volumes SET last_scan_status='RUNNING' WHERE id='outer'",
            []
        ))
        .is_err()
    );
    assert!(
        db.record_volume_scan("outer", FileIndexScanStatus::Completed, -1, Some(2))
            .is_err()
    );
}
