use super::{Result, Volume, linux, mounts};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
};

struct Snapshot {
    mount_info: PathBuf,
    uuid_directory: PathBuf,
    boot_id: PathBuf,
}

thread_local! {
    static SNAPSHOT: RefCell<Option<Snapshot>> = const { RefCell::new(None) };
}

pub fn with_snapshot<T>(
    mount_info: &Path,
    uuid_directory: &Path,
    boot_id: &Path,
    run: impl FnOnce() -> T,
) -> T {
    struct Restore(Option<Snapshot>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SNAPSHOT.with(|snapshot| snapshot.replace(self.0.take()));
        }
    }
    let _restore = Restore(SNAPSHOT.with(|snapshot| {
        snapshot.replace(Some(Snapshot {
            mount_info: mount_info.to_owned(),
            uuid_directory: uuid_directory.to_owned(),
            boot_id: boot_id.to_owned(),
        }))
    }));
    run()
}

pub(super) fn containing(path: &Path) -> Option<Result<Volume>> {
    SNAPSHOT.with(|snapshot| {
        snapshot.borrow().as_ref().map(|snapshot| {
            let mounts = std::fs::read_to_string(&snapshot.mount_info)
                .map_err(Into::into)
                .and_then(|input| mounts::parse(&input))?;
            linux::containing_from_mounts(path, mounts, &snapshot.uuid_directory, &snapshot.boot_id)
        })
    })
}
