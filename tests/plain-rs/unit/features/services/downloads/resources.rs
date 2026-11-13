use super::*;
use crate::features::{
    downloads::{Download, Source, Status},
    files::resource_link::{self, Resource, Scope},
};
use std::{path::Path, sync::Arc};

fn fixture(path: &Path) -> crate::test_runtime::Core {
    std::fs::create_dir_all(path).unwrap();
    let prefs = Arc::new(crate::features::prefs::Prefs::load(&path.join("prefs.json")).unwrap());
    prefs.set("client_id", "fixture").unwrap();
    crate::test_runtime::Core::start(path, &plain_libs::crypto::gen_token(), prefs).unwrap()
}

fn resource(state: &CoreRuntime, path: &Path) -> String {
    let session = crate::features::services::sessions::native(state).unwrap();
    let id = resource_link::seal(
        &Scope::Session(session.client_id),
        &plain_libs::utils::base64::base64_decode_checked(&session.url_token).unwrap(),
        &Resource::reference(path.to_str().unwrap()),
    )
    .unwrap();
    crate::features::services::files::links::download_url(&id)
}

async fn terminal(state: &CoreRuntime, id: &str) -> Download {
    let mut changed = state.downloads.changed.subscribe();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let task = state
                .downloads
                .snapshot()
                .into_iter()
                .find(|task| task.id == id)
                .unwrap();
            if task.status.terminal() {
                return task;
            }
            changed.changed().await.unwrap();
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn normal_resource_creation_preserves_tree_empty_entries_bytes_and_existing_destinations() {
    let root = crate::test_runtime::public_directory();
    let core = fixture(&root.path().join("state"));
    let state = core.runtime_state();
    let source = root.path().join("Root");
    std::fs::create_dir_all(source.join("nested/empty-dir")).unwrap();
    let bytes = [0, 255, 128, 3];
    std::fs::write(source.join("nested/文件.bin"), bytes).unwrap();
    std::fs::write(source.join("empty.bin"), []).unwrap();
    let target = root.path().join("out");
    std::fs::create_dir_all(target.join("Root/nested")).unwrap();
    std::fs::write(target.join("Root/nested/文件.bin"), b"keep").unwrap();
    let task = super::super::create(
        &state,
        Source::Resources {
            urls: vec![resource(&state, &source)],
        },
        Some(target.to_str().unwrap().into()),
        None,
    )
    .await
    .unwrap();
    let task = terminal(&state, &task.id).await;
    assert_eq!(task.status, Status::Completed, "{}", task.error);
    assert_eq!(
        (
            task.total_files,
            task.done_files,
            task.total_bytes,
            task.downloaded_bytes
        ),
        (2, 2, 4, 4)
    );
    assert_eq!(
        std::fs::read(target.join("Root/nested/文件.bin")).unwrap(),
        b"keep"
    );
    assert_eq!(
        std::fs::read(target.join("Root/nested/文件 (1).bin")).unwrap(),
        bytes
    );
    assert_eq!(
        std::fs::metadata(target.join("Root/empty.bin"))
            .unwrap()
            .len(),
        0
    );
    assert!(target.join("Root/nested/empty-dir").is_dir());
    core.shutdown().await;
}

#[tokio::test]
async fn the_same_resource_source_can_be_saved_as_a_zip_with_empty_directories() {
    let root = crate::test_runtime::public_directory();
    let core = fixture(&root.path().join("state"));
    let state = core.runtime_state();
    let source = root.path().join("Root");
    std::fs::create_dir_all(source.join("empty")).unwrap();
    std::fs::write(source.join("file.bin"), b"original").unwrap();
    let target = root.path().join("out");
    let task = super::super::create(
        &state,
        Source::Resources {
            urls: vec![resource(&state, &source)],
        },
        Some(target.to_str().unwrap().into()),
        Some("download.zip".into()),
    )
    .await
    .unwrap();
    let task = terminal(&state, &task.id).await;
    assert_eq!(task.status, Status::Completed, "{}", task.error);
    let mut zip =
        plain_libs::zip::ZipArchive::new(std::fs::File::open(target.join("download.zip")).unwrap())
            .unwrap();
    assert!(zip.by_name("Root/empty/").unwrap().is_dir());
    let mut bytes = vec![];
    std::io::Read::read_to_end(&mut zip.by_name("Root/file.bin").unwrap(), &mut bytes).unwrap();
    assert_eq!(bytes, b"original");
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 1);
    core.shutdown().await;
}

#[tokio::test]
async fn invalid_resource_contracts_fail_before_creating_tasks_or_destinations() {
    let root = crate::test_runtime::public_directory();
    let core = fixture(&root.path().join("state"));
    let state = core.runtime_state();
    let target = root.path().join("out");
    for urls in [
        vec![],
        vec!["http://fixture.invalid/fs?id=s.share.AA&action=download".into()],
        vec!["https://fixture.invalid/fs?id=s.share.AA&action=preview".into()],
        vec!["https://fixture.invalid/fs?id=s.share.AA&action=download&size=sm".into()],
    ] {
        assert!(
            super::super::create(
                &state,
                Source::Resources { urls },
                Some(target.to_str().unwrap().into()),
                None
            )
            .await
            .is_err()
        );
        assert!(state.downloads.snapshot().is_empty());
        assert!(!target.exists());
    }
    core.shutdown().await;
}

#[tokio::test]
async fn completed_workers_release_capacity_for_the_next_queued_resource() {
    let root = crate::test_runtime::public_directory();
    let core = fixture(&root.path().join("state"));
    let state = core.runtime_state();
    let target = root.path().join("out");
    let mut ids = vec![];
    for number in 0..4 {
        let source = root.path().join(format!("file-{number}.bin"));
        std::fs::write(&source, [number as u8; 3]).unwrap();
        let task = super::super::create(
            &state,
            Source::Resources {
                urls: vec![resource(&state, &source)],
            },
            Some(target.to_str().unwrap().into()),
            None,
        )
        .await
        .unwrap();
        ids.push(task.id);
    }
    for (number, id) in ids.iter().enumerate() {
        let task = terminal(&state, id).await;
        assert_eq!(task.status, Status::Completed, "{}", task.error);
        assert_eq!(
            std::fs::read(target.join(format!("file-{number}.bin"))).unwrap(),
            [number as u8; 3]
        );
    }
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 4);
    core.shutdown().await;
}
