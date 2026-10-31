use crate::server::ContentServer;
use async_graphql::Request;
use serde_json::json;

async fn rename(
    core: &ContentServer,
    path: &std::path::Path,
    name: &str,
) -> async_graphql::Response {
    core.native_graphql()
        .unwrap()
        .execute(
            Request::new(
                "mutation($path:String!,$name:String!){renameFile(path:$path,name:$name)}",
            )
            .variables(async_graphql::Variables::from_json(
                json!({"path":path,"name":name}),
            )),
        )
        .await
}

fn relation(core: &ContentServer, tag: &str) -> Option<String> {
    core.runtime_state()
        .core
        .db
        .with_conn(|connection| {
            use rusqlite::OptionalExtension;
            connection
                .query_row(
                    "SELECT target_key FROM tag_relations WHERE tag_id=?1",
                    [tag],
                    |row| row.get(0),
                )
                .optional()
        })
        .unwrap()
}

#[tokio::test]
async fn strict_native_rename_and_delete_rebind_and_clean_generic_file_references_without_host() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state().core;
    let (_generation, mut requests) = state.host.connect();
    let source = directory.path().join("source.txt");
    let collision = directory.path().join("existing.txt");
    let destination = directory.path().join("renamed.txt");
    std::fs::write(&source, b"source").unwrap();
    std::fs::write(&collision, b"existing").unwrap();
    state.db.with_conn(|connection| connection.execute(
        "INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('file',?1,22,'first')", [source.to_str().unwrap()]
    )).unwrap();
    for name in ["existing.txt", "..", "../escape", ""] {
        let response = rename(&core, &source, name).await;
        assert!(response.errors.is_empty(), "{name}: {:?}", response.errors);
        assert!(response.data.into_json().unwrap()["renameFile"].is_null());
        assert_eq!(std::fs::read(&source).unwrap(), b"source");
        assert_eq!(std::fs::read(&collision).unwrap(), b"existing");
    }
    let response = rename(&core, &source, "renamed.txt").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["renameFile"],
        destination.to_str().unwrap()
    );
    assert!(!source.exists());
    assert_eq!(std::fs::read(&destination).unwrap(), b"source");
    assert_eq!(
        relation(&core, "file"),
        Some(destination.to_str().unwrap().into())
    );
    let deleted = core
        .native_graphql()
        .unwrap()
        .execute(
            Request::new("mutation($paths:[String!]!){deleteFiles(paths:$paths){affectedCount}}")
                .variables(async_graphql::Variables::from_json(
                    json!({"paths":[destination]}),
                )),
        )
        .await;
    assert!(deleted.errors.is_empty(), "{:?}", deleted.errors);
    assert_eq!(
        deleted.data.into_json().unwrap()["deleteFiles"]["affectedCount"],
        1
    );
    assert!(!destination.exists());
    assert!(relation(&core, "file").is_none());
    assert!(
        !state
            .files
            .delete(destination.to_str().unwrap().into())
            .await
            .unwrap()
            .removed
    );
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn rename_postprocessing_failure_preserves_physical_receipt_and_does_not_retry_after_restart()
{
    let directory = tempfile::tempdir().unwrap();
    let core_directory = directory.path().join("core");
    let first = ContentServer::open(&core_directory).unwrap();
    let state = first.runtime_state().core;
    let source = directory.path().join("source.txt");
    let destination = directory.path().join("renamed.txt");
    std::fs::write(&source, b"fixture").unwrap();
    state.db.with_conn(|connection| connection.execute(
        "INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('file',?1,22,'first')", [source.to_str().unwrap()]
    )).unwrap();
    state.media_scans.close().unwrap();
    let response = rename(&first, &source, "renamed.txt").await;
    assert_eq!(response.errors.len(), 1);
    assert!(
        response.errors[0]
            .message
            .contains("rename postprocessing failed")
    );
    assert!(!source.exists());
    assert_eq!(std::fs::read(&destination).unwrap(), b"fixture");
    let owner = state.native_client_id().unwrap();
    let failed = state.files.list(owner.clone()).await.unwrap().remove(0);
    assert_eq!(failed.status, plain_rs::files::tasks::FileTaskStatus::Error);
    assert_eq!(failed.completed_ops[0].dst, destination.to_str().unwrap());
    assert_eq!(
        relation(&first, "file"),
        Some(source.to_str().unwrap().into())
    );
    first.shutdown().await;
    drop(state);
    let second = ContentServer::open(&core_directory).unwrap();
    assert!(
        second
            .runtime_state()
            .core
            .files
            .list(owner)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        relation(&second, "file"),
        Some(source.to_str().unwrap().into())
    );
    assert_eq!(std::fs::read(&destination).unwrap(), b"fixture");
    assert!(!directory.path().join("renamed (1).txt").exists());
    second.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn native_rename_and_delete_follow_actual_catalog_identities() {
    use plain_rs::enums::DataType;
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let directory = tempfile::Builder::new()
        .prefix("plain-files-")
        .tempdir_in(home)
        .unwrap();
    let directory_path = directory.path().canonicalize().unwrap();
    let core = ContentServer::open(&directory_path.join("core")).unwrap();
    let state = core.runtime_state().core;
    let source = directory_path.join("source.png");
    let destination = directory_path.join("renamed.png");
    image::RgbImage::from_pixel(8, 8, image::Rgb([20, 40, 60]))
        .save(&source)
        .unwrap();
    let read =
        plain_rs::media::file_facts::read_file_metadata_with_availability(&source, DataType::Image)
            .unwrap();
    let rows = core
        .native_media_catalog()
        .unwrap()
        .record_files(&[read.metadata])
        .unwrap();
    state.db.with_conn(|connection| connection.execute(
        "INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('image',?1,3,'first')", [&rows[0].id]
    )).unwrap();
    let (_generation, mut requests) = state.host.connect();
    let response = rename(&core, &source, "renamed.png").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["renameFile"],
        destination.to_str().unwrap()
    );
    let current = state
        .db
        .media_file_by_path(DataType::Image, destination.to_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(relation(&core, "image"), Some(current.id));
    assert!(
        state
            .db
            .media_file_by_path(DataType::Image, source.to_str().unwrap())
            .unwrap()
            .is_none()
    );
    assert!(
        state
            .files
            .delete(destination.to_str().unwrap().into())
            .await
            .unwrap()
            .removed
    );
    assert!(relation(&core, "image").is_none());
    assert!(!destination.exists());
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn named_copy_creates_parents_and_explicit_copy_overwrite_keeps_the_request_owner_without_host()
 {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state().core;
    let (_generation, mut requests) = state.host.connect();
    let mut events = state.events.subscribe();
    let source = directory.path().join("source +.txt");
    let target = directory.path().join("new/nested");
    let destination = target.join("saved.txt");
    std::fs::write(&source, b"copied bytes").unwrap();
    let native = core.native_graphql().unwrap();
    let response = native.execute(Request::new("mutation($src:String!,$directory:String!){copyFileToDirectory(src:$src,directory:$directory,name:\"saved.txt\")}")
        .variables(async_graphql::Variables::from_json(json!({"src":format!("file://{}",source.to_str().unwrap()),"directory":target})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["copyFileToDirectory"],
        destination.to_string_lossy().as_ref()
    );
    assert_eq!(std::fs::read(&destination).unwrap(), b"copied bytes");
    std::fs::write(&source, b"replacement").unwrap();
    let response = native
        .execute(
            Request::new(
                "mutation($src:String!,$dst:String!){copyFile(src:$src,dst:$dst,overwrite:true)}",
            )
            .variables(async_graphql::Variables::from_json(
                json!({"src":source,"dst":destination}),
            )),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["copyFile"], true);
    assert_eq!(std::fs::read(&destination).unwrap(), b"replacement");
    assert!(!target.join("saved (1).txt").exists());
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut content_changes = 0;
        while content_changes < 2 {
            if events.recv().await.unwrap().event_type == "CONTENT_CHANGED" {
                content_changes += 1;
            }
        }
    })
    .await
    .unwrap();
    while let Ok(event) = events.try_recv() {
        assert_ne!(
            event.event_type, "CONTENT_CHANGED",
            "no duplicate executor broadcast"
        );
    }
    let owner = state
        .prefs
        .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)
        .unwrap()
        .unwrap();
    let rows = state.files.list(owner.clone()).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|task| task.client_id == owner));
    state
        .prefs
        .set("api_permissions", json!(["WRITE_EXTERNAL_STORAGE"]))
        .unwrap();
    let foreign_destination = target.join("foreign.txt");
    let response = core
        .runtime_state()
        .public
        .execute(
            Request::new(
                "mutation($src:String!,$dst:String!){copyFile(src:$src,dst:$dst,overwrite:true)}",
            )
            .data("copy-request-owner".to_owned())
            .variables(async_graphql::Variables::from_json(
                json!({"src":source,"dst":foreign_destination}),
            )),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["copyFile"], true);
    let foreign = state
        .files
        .list("copy-request-owner".to_owned())
        .await
        .unwrap();
    assert_eq!(foreign.len(), 1);
    assert_eq!(foreign[0].client_id, "copy-request-owner");
    assert_eq!(state.files.list(owner).await.unwrap().len(), 2);
    assert_eq!(std::fs::read(foreign_destination).unwrap(), b"replacement");
    for name in ["", "..", "../outside", "a/b", "a\\b"] {
        let response = native.execute(Request::new("mutation($src:String!,$directory:String!,$name:String!){copyFileToDirectory(src:$src,directory:$directory,name:$name)}")
            .variables(async_graphql::Variables::from_json(json!({"src":source,"directory":target,"name":name})))).await;
        assert!(!response.errors.is_empty(), "{name}");
    }
    assert!(!directory.path().join("outside").exists());
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn main_batch_delete_validates_all_paths_before_effects_and_counts_actual_roots_once() {
    let temp = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&temp.path().join("core")).unwrap();
    let state = core.runtime_state().core;
    let (_generation, mut host_requests) = state.host.connect();
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    let absent = temp.path().join("absent");
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&second, b"second").unwrap();
    let native = core.native_graphql().unwrap();
    let document = "mutation($paths:[String!]!){deleteFiles(paths:$paths){affectedCount}}";
    for invalid in ["relative", "/"] {
        let response = native
            .execute(
                Request::new(document).variables(async_graphql::Variables::from_json(
                    json!({"paths":[first,invalid]}),
                )),
            )
            .await;
        assert!(!response.errors.is_empty());
        assert_eq!(std::fs::read(&first).unwrap(), b"first");
        assert_eq!(std::fs::read(&second).unwrap(), b"second");
    }
    let response = native
        .execute(
            Request::new(document).variables(async_graphql::Variables::from_json(
                json!({"paths":[first,absent,first,second]}),
            )),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["deleteFiles"]["affectedCount"],
        2
    );
    assert!(!first.exists());
    assert!(!second.exists());
    assert_eq!(state.files.delete_many(vec![]).await.unwrap(), 0);
    assert!(host_requests.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn named_copy_preserves_existing_files_and_returns_each_actual_collision_destination() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state().core;
    let (_generation, mut requests) = state.host.connect();
    let source = directory.path().join("source.txt");
    let target = directory.path().join("exports");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(&source, b"new export").unwrap();
    std::fs::write(target.join("saved.txt"), b"existing personal file").unwrap();
    std::fs::write(target.join("saved (1).txt"), b"existing first copy").unwrap();
    let native = core.native_graphql().unwrap();
    for suffix in [2, 3] {
        let response = native.execute(Request::new(
            "mutation($src:String!,$directory:String!){copyFileToDirectory(src:$src,directory:$directory,name:\"saved.txt\")}",
        ).variables(async_graphql::Variables::from_json(json!({"src":source,"directory":target})))).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let destination = target.join(format!("saved ({suffix}).txt"));
        assert_eq!(
            response.data.into_json().unwrap()["copyFileToDirectory"],
            destination.to_string_lossy().as_ref()
        );
        assert_eq!(std::fs::read(destination).unwrap(), b"new export");
        assert_eq!(
            std::fs::read(target.join("saved.txt")).unwrap(),
            b"existing personal file"
        );
        assert_eq!(
            std::fs::read(target.join("saved (1).txt")).unwrap(),
            b"existing first copy"
        );
        assert_eq!(std::fs::read(&source).unwrap(), b"new export");
    }
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn named_copy_does_not_treat_an_existing_named_directory_as_the_destination_container() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state().core;
    let (_generation, mut requests) = state.host.connect();
    let source = directory.path().join("source.bin");
    let target = directory.path().join("exports");
    let existing = target.join("saved.bin");
    std::fs::create_dir_all(&existing).unwrap();
    std::fs::write(&source, b"export").unwrap();
    std::fs::write(existing.join("source.bin"), b"existing content").unwrap();
    let response = core.native_graphql().unwrap().execute(Request::new(
        "mutation($src:String!,$directory:String!){copyFileToDirectory(src:$src,directory:$directory,name:\"saved.bin\")}",
    ).variables(async_graphql::Variables::from_json(json!({"src":source,"directory":target})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let actual = target.join("saved (1).bin");
    assert_eq!(
        response.data.into_json().unwrap()["copyFileToDirectory"],
        actual.to_string_lossy().as_ref()
    );
    assert_eq!(std::fs::read(actual).unwrap(), b"export");
    assert_eq!(
        std::fs::read(existing.join("source.bin")).unwrap(),
        b"existing content"
    );
    assert_eq!(std::fs::read_dir(existing).unwrap().count(), 1);
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}
