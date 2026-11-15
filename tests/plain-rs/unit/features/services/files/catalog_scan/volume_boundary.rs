use super::*;
use std::os::unix::fs::MetadataExt;

fn with_volumes<F, Work>(run: F)
where
    F: FnOnce(
        crate::runtime::Runtime,
        NativeMediaCatalog,
        Vec<std::path::PathBuf>,
        Vec<String>,
    ) -> Work,
    Work: std::future::Future<Output = ()>,
{
    let fixture = directory_scan_fixture();
    let root = fixture.path().join("internal");
    let usb1 = root.join("mnt/usb1");
    let usb2 = root.join("mnt/usb2");
    for path in [&root, &usb1, &usb2] {
        std::fs::create_dir_all(path).unwrap();
        std::fs::write(path.join("generated.rs"), b"generated volume fixture").unwrap();
    }
    let root = root.canonicalize().unwrap();
    let paths = [root.clone(), root.join("mnt/usb1"), root.join("mnt/usb2")];
    let uuids = fixture.path().join("by-uuid");
    std::fs::create_dir(&uuids).unwrap();
    let boot = fixture.path().join("boot-id");
    std::fs::write(&boot, "generated-boot").unwrap();
    let mut mountinfo = String::new();
    for (index, (path, device)) in paths
        .iter()
        .zip(["/dev/null", "/dev/zero", "/dev/random"])
        .enumerate()
    {
        let uuid = format!("generated-volume-{index}");
        std::os::unix::fs::symlink(device, uuids.join(uuid)).unwrap();
        let number = std::fs::metadata(device).unwrap().rdev() as libc::dev_t;
        let escaped = path
            .to_str()
            .unwrap()
            .replace('\\', "\\134")
            .replace(' ', "\\040");
        mountinfo.push_str(&format!(
            "{} 1 {}:{} / {escaped} rw - ext4 /dev/generated-{index} rw\n",
            index + 10,
            libc::major(number),
            libc::minor(number)
        ));
    }
    let mounts = fixture.path().join("mountinfo");
    std::fs::write(&mounts, mountinfo).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    plain_libs::platform::volumes::test_mounts::with_snapshot(&mounts, &uuids, &boot, || {
        runtime.block_on(async {
            let core = crate::test_runtime::Core::open(&fixture.path().join("core")).unwrap();
            let state = core.runtime_state();
            let catalog = NativeMediaCatalog::new(state.clone());
            let mut ids = Vec::new();
            for path in &paths {
                let id = state.db.register_volume(path).unwrap().unwrap();
                state
                    .db
                    .set_volume_index(
                        &id,
                        crate::infrastructure::db::FileIndexMode::Blocklist,
                        &[],
                        &[],
                    )
                    .unwrap();
                ids.push(id);
            }
            catalog.configure_scan_directories(None).unwrap();
            run(state, catalog, paths.to_vec(), ids).await;
            core.stop_core().await;
        })
    });
}

#[test]
fn rebuild_all_plans_each_volume_and_a_single_volume_preserves_its_neighbors() {
    with_volumes(|state, catalog, _paths, ids| async move {
        catalog.start_volume_scan(None).unwrap().unwrap();
        assert!(state.media_scans.wait_idle().await.unwrap());
        let result = state.media_scans.snapshot().unwrap().unwrap();
        assert_eq!(
            result.status,
            crate::features::services::files::media_scans::Status::Completed,
            "{:?}",
            result.error
        );
        assert_eq!(
            catalog
                .directory_walks
                .load(std::sync::atomic::Ordering::Relaxed),
            3,
            "all-volume rebuild must plan each volume separately"
        );
        assert_eq!(result.total_files, Some(1));
        assert_eq!(result.processed_files, 1);
        for volume in state.db.volumes().unwrap() {
            assert_eq!(volume.last_scan_count, Some(1));
            assert_eq!(volume.last_scan_total, Some(1));
        }
        catalog
            .directory_walks
            .store(0, std::sync::atomic::Ordering::Relaxed);
        catalog.start_volume_scan(Some(&ids[0])).unwrap().unwrap();
        assert!(state.media_scans.wait_idle().await.unwrap());
        let result = state.media_scans.snapshot().unwrap().unwrap();
        assert_eq!(result.total_files, Some(1));
        assert_eq!(result.processed_files, 1);
        assert_eq!(
            catalog
                .directory_walks
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        for id in &ids[1..] {
            let count: i64 = state
                .db
                .with_conn(|conn| {
                    conn.query_row(
                        "SELECT count(*) FROM files WHERE volume_id=?1 AND kind='FILE'",
                        [id],
                        |row| row.get(0),
                    )
                })
                .unwrap();
            assert_eq!(
                count, 1,
                "rebuilding the internal volume must preserve each other volume"
            );
        }
    });
}

#[test]
fn configuring_a_second_volume_preserves_the_first_scan_while_watcher_registration_is_pending() {
    with_volumes(|state, catalog, paths, ids| async move {
        for id in &ids[1..] {
            state
                .db
                .set_volume_index(
                    id,
                    crate::infrastructure::db::FileIndexMode::Unconfigured,
                    &[],
                    &[],
                )
                .unwrap();
        }
        catalog.configure_scan_directories(None).unwrap();
        let (watcher, entered, release) =
            crate::features::services::files::media_watch::tests::pending_watcher(&paths[0]);
        state
            .media_watch
            .lock()
            .unwrap()
            .insert(ids[0].clone(), watcher);
        entered.await.unwrap();
        let first = catalog.start_volume_scan(Some(&ids[0])).unwrap().unwrap();
        // Yield until the normal scanner has subscribed to the pending watcher.
        tokio::task::yield_now().await;
        crate::features::services::files::indexing::set(
            &state,
            &ids[1],
            crate::infrastructure::db::FileIndexMode::Blocklist,
            vec![],
            vec![],
        )
        .await
        .unwrap();
        let running = state.media_scans.snapshot_for(&ids[0]).unwrap().unwrap();
        let queued = state.media_scans.snapshot_for(&ids[1]).unwrap().unwrap();
        assert_eq!(
            running.status,
            crate::features::services::files::media_scans::Status::Running
        );
        assert_eq!(
            queued.status,
            crate::features::services::files::media_scans::Status::Queued
        );
        assert_eq!(queued.processed_files, 0);
        release.send(()).unwrap();
        state.media_scans.wait_background().await.unwrap();
        let result = state.media_scans.snapshot_for(&ids[0]).unwrap().unwrap();
        assert_eq!(
            result.id, first.id,
            "configuring another volume must preserve the task"
        );
        assert_eq!(
            result.status,
            crate::features::services::files::media_scans::Status::Completed,
            "{:?}",
            result.error
        );
        assert_eq!(result.processed_files, 1);
        let second = state.media_scans.snapshot_for(&ids[1]).unwrap().unwrap();
        assert_eq!(
            second.status,
            crate::features::services::files::media_scans::Status::Completed,
            "{:?}",
            second.error
        );
        assert_eq!(second.processed_files, 1);
    });
}
