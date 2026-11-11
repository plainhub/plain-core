use super::super::*;
use std::os::unix::fs::MetadataExt;

#[test]
fn uuid_loss_and_recovery_preserve_registered_volume_files_and_index_configuration() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let disk = root.path().join("generated disk");
    std::fs::create_dir_all(disk.join("Photos")).unwrap();
    let disk = disk.canonicalize().unwrap();
    let file = disk.join("Photos/generated.txt");
    let contents = b"independently generated registration fixture";
    std::fs::write(&file, contents).unwrap();
    let uuids = root.path().join("by-uuid");
    std::fs::create_dir(&uuids).unwrap();
    let uuid = uuids.join("generated-registration-volume");
    std::os::unix::fs::symlink("/dev/null", &uuid).unwrap();
    let boot = root.path().join("boot-id");
    std::fs::write(&boot, "generated-boot").unwrap();
    let mount_info = root.path().join("mountinfo");
    let device = std::fs::metadata("/dev/null").unwrap().rdev() as libc::dev_t;
    let escaped = disk
        .to_str()
        .unwrap()
        .replace('\\', "\\134")
        .replace(' ', "\\040");
    std::fs::write(
        &mount_info,
        format!(
            "47 1 {}:{} / {escaped} rw - ext4 /dev/generated-drive rw\n",
            libc::major(device),
            libc::minor(device)
        ),
    )
    .unwrap();

    plain_libs::platform::volumes::test_mounts::with_snapshot(&mount_info, &uuids, &boot, || {
        let db = Db::open(&root.path().join("index.db")).unwrap();
        let id = db.register_volume(&disk).unwrap().unwrap();
        db.set_volume_index(
            &id,
            crate::infrastructure::db::IndexMode::Allowlist,
            &["Photos".into()],
            &["Photos/Excluded".into()],
        )
        .unwrap();
        let file_row = db
            .record_file_paths(&[file.to_str().unwrap().into()], None)
            .unwrap()
            .remove(0);
        assert_eq!(file_row.volume_id, id);
        db.with_conn(|conn| {
            conn.execute("UPDATE files SET favorited=1 WHERE id=?1", [&file_row.id])
        })
        .unwrap();
        let association = || {
            db.with_conn(|conn| {
                conn.query_row(
                    "SELECT id,volume_id,path,favorited,missing,indexed FROM files WHERE id=?1",
                    [&file_row.id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, bool>(3)?,
                            row.get::<_, bool>(4)?,
                            row.get::<_, bool>(5)?,
                        ))
                    },
                )
            })
            .unwrap()
        };
        let original_file = association();
        assert_eq!(original_file.1, id);
        assert_eq!(original_file.2, "Photos/generated.txt");
        assert!(original_file.3);
        assert!(!original_file.4);
        let original_volumes = db.volumes().unwrap();
        assert_eq!(original_volumes.len(), 1);
        assert_eq!(
            original_volumes[0].identity,
            "UUID:GENERATED-REGISTRATION-VOLUME"
        );
        assert!(original_volumes[0].is_available);
        assert_eq!(original_volumes[0].id, id);
        assert_eq!(
            original_volumes[0].index_mode,
            crate::infrastructure::db::IndexMode::Allowlist
        );
        assert_eq!(original_volumes[0].included_dirs, ["Photos"]);
        assert_eq!(original_volumes[0].excluded_dirs, ["Photos/Excluded"]);

        for _ in 0..2 {
            std::fs::remove_file(&uuid).unwrap();
            let registered = db.register_volume(&disk).unwrap();
            assert_eq!(
                db.volumes().unwrap().len(),
                1,
                "UUID loss must not insert another volume"
            );
            assert_eq!(registered, None);
            db.refresh_volume_availability().unwrap();
            let mut offline = original_volumes.clone();
            offline[0].is_available = false;
            assert_eq!(db.volumes().unwrap(), offline);
            assert_eq!(association(), original_file);

            std::os::unix::fs::symlink("/dev/null", &uuid).unwrap();
            assert_eq!(db.register_volume(&disk).unwrap(), Some(id.clone()));
            db.refresh_volume_availability().unwrap();
            assert_eq!(db.volumes().unwrap(), original_volumes);
            assert_eq!(association(), original_file);
            assert_eq!(
                db.record_file_paths(&[file.to_str().unwrap().into()], None)
                    .unwrap()
                    .remove(0)
                    .id,
                file_row.id
            );
            assert_eq!(association(), original_file);
        }
        db.with_conn(|conn| {
            assert_eq!(
                conn.query_row("SELECT count(*) FROM files", [], |row| row.get::<_, i64>(0))?,
                1
            );
            assert_eq!(
                conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get::<_, i64>(0)
                })?,
                0
            );
            Ok::<_, plain_libs::rusqlite::Error>(())
        })
        .unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), contents);
    });
}
