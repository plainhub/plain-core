use super::*;
use crate::graphql::schemas::public::PublicSchema;
use crate::host::bridge::Host;
use plain_rs::db::Db;
use plain_rs::prefs::Prefs;
use serde_json::{Value, json};
use std::sync::Arc;

const GRANTED: &str = r#"["WRITE_EXTERNAL_STORAGE"]"#;

fn stub(host: Arc<Host>, handler: impl Fn(&str, Value) -> Value + Send + 'static) {
    let (generation, mut requests) = host.connect();
    let host = host.clone();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let Some(id) = request["id"].as_u64() else {
                continue;
            };
            let result = handler(
                &request["method"].as_str().unwrap_or_default(),
                request["params"].clone(),
            );
            let _ = host.reply(generation, json!({ "id": id, "result": result }));
        }
    });
}

fn fixture(permissions: &str) -> (tempfile::TempDir, PublicSchema) {
    fixture_with(permissions, |method, _| match method {
        "systemPermissionFacts" => json!({ "granted": { "WRITE_EXTERNAL_STORAGE": true } }),
        other => panic!("unexpected host call {other}"),
    })
}

fn fixture_with(
    permissions: &str,
    handler: impl Fn(&str, Value) -> Value + Send + 'static,
) -> (tempfile::TempDir, PublicSchema) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs
        .set(
            "api_permissions",
            json!(serde_json::from_str::<Vec<String>>(permissions).unwrap()),
        )
        .unwrap();
    prefs.set("client_id", "file-test").unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let host = Arc::new(Host::default());
    stub(host.clone(), handler);
    let directory = dir.path().to_path_buf();
    let (events, _) = tokio::sync::broadcast::channel(16);
    (
        dir,
        crate::graphql::schemas::public::build(host, events, prefs, db, directory),
    )
}

fn database(schema: &PublicSchema) -> &Arc<Db> {
    schema.data::<Arc<Db>>().unwrap()
}

fn seed_media(
    schema: &PublicSchema,
    data_type: plain_rs::enums::DataType,
    count: usize,
    trashed: bool,
    extension: &str,
) -> Vec<plain_rs::db::MediaFileRow> {
    let base = schema
        .data::<Arc<Prefs>>()
        .unwrap()
        .path()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap()
        .join("media");
    std::fs::create_dir_all(&base).unwrap();
    let rows: Vec<_> = (1..=count)
        .map(|i| plain_rs::db::MediaFileMetadata {
            album: String::new(),
            album_artist: String::new(),

            data_type,
            path: {
                let path = base.join(format!("x{i}.{extension}"));
                std::fs::write(&path, b"fixture").unwrap();
                path.to_string_lossy().into_owned()
            },
            title: format!("x{i}"),
            artist: String::new(),
            bucket_id: "b".to_owned(),
            size_bytes: 10,
            duration_ms: 0,
            cover_image_id: Some(String::new()),
            width: 0,
            height: 0,
            rotation: 0,
            taken_at: None,
            created_at: "2026-01-01T00:00:00.000Z".to_owned(),
            updated_at: "2026-01-01T00:00:00.000Z".to_owned(),
        })
        .collect();
    let recorded = database(schema).record_media_files(&rows).unwrap();
    if trashed {
        database(schema).with_conn(|conn| {
            for row in &recorded {
                conn.execute("UPDATE files SET trashed_at='2026-01-01T00:00:00Z',original_path=path WHERE id=?1", [&row.id])
                    .unwrap();
            }
        });
    }
    recorded
}

#[tokio::test]
async fn file_writes_are_gated_on_storage_permission() {
    let (_dir, schema) = fixture("[]");
    let response = schema
        .execute(
            r#"mutation { deleteFiles(paths:["/a"]) { affectedCount }
                      createDir(path:"/a/b") { name }
                      renameFile(path:"/a", name:"c")
                      writeTextFile(path:"/a/b.txt", content:"x", overwrite:false) { name }
                      copyFile(src:"/a", dst:"/b", overwrite:false)
                      moveFile(src:"/a", dst:"/b", overwrite:false) }"#,
        )
        .await;
    assert_eq!(response.errors.len(), 6);
    for error in &response.errors {
        assert_eq!(error.message, "no_permission");
    }
}

/// The count is the delta, not the request size: a path that was already
/// gone did not delete anything.
#[tokio::test]
async fn delete_files_reports_what_was_actually_removed() {
    let (dir, schema) = fixture(GRANTED);
    let paths = ["a", "b", "missing"].map(|name| dir.path().join(name));
    for path in &paths[..2] {
        std::fs::write(path, "value").unwrap();
    }
    let response = schema
        .execute(
            async_graphql::Request::new(
                "mutation($paths:[String!]!) {deleteFiles(paths:$paths){affectedCount}}",
            )
            .variables(async_graphql::Variables::from_json(json!({"paths":paths}))),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["deleteFiles"]["affectedCount"],
        2
    );
    assert!(paths.iter().all(|path| !path.exists()));
}

#[tokio::test]
async fn deleting_no_paths_is_a_no_op() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _| match method {
        "systemPermissionFacts" => json!({ "granted": { "WRITE_EXTERNAL_STORAGE": true } }),
        other => panic!("the platform must not be asked: {other}"),
    });
    let response = schema
        .execute(r#"mutation { deleteFiles(paths:[]) { affectedCount } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["deleteFiles"]["affectedCount"],
        0
    );
}

#[tokio::test]
async fn a_created_directory_reads_back_as_a_file_row() {
    let (dir, schema) = fixture(GRANTED);
    let path = dir.path().join("DCIM");
    let response = schema.execute(async_graphql::Request::new("mutation($path:String!){createDir(path:$path){name path isDir childCount mediaId updatedAt}}")
        .variables(async_graphql::Variables::from_json(json!({"path":path})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let value = response.data.into_json().unwrap();
    let row = &value["createDir"];
    assert_eq!(row["name"], "DCIM");
    assert_eq!(row["path"], json!(path));
    assert_eq!(row["isDir"], true);
    assert_eq!(row["childCount"], 0);
    assert!(row["mediaId"].is_null());
    assert!(chrono::DateTime::parse_from_rfc3339(row["updatedAt"].as_str().unwrap()).is_ok());
    assert!(path.is_dir());
}

#[tokio::test]
async fn a_refused_rename_reports_false() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _| match method {
        "systemPermissionFacts" => json!({ "granted": { "WRITE_EXTERNAL_STORAGE": true } }),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(
            async_graphql::Request::new(r#"mutation { renameFile(path:"/a", name:"..") }"#)
                .data("file-test".to_owned()),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(response.data.into_json().unwrap()["renameFile"].is_null());
}

#[tokio::test]
async fn copy_and_move_use_the_shared_file_tasks() {
    let (dir, schema) = fixture(GRANTED);
    let source = dir.path().join("a");
    let destination = dir.path().join("b");
    for name in ["copyFile", "moveFile"] {
        std::fs::write(&source, "new").unwrap();
        std::fs::write(&destination, "old").unwrap();
        let response = schema.execute(async_graphql::Request::new(format!("mutation($src:String!,$dst:String!){{{name}(src:$src,dst:$dst,overwrite:true)}}"))
            .variables(async_graphql::Variables::from_json(json!({"src":source,"dst":destination}))).data("file-test".to_owned())).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()[name], true);
        assert_eq!(std::fs::read_to_string(&destination).unwrap(), "new");
        assert_eq!(source.exists(), name == "copyFile");
        let tasks = schema
            .data::<Arc<crate::services::files::file_tasks::FileTasks>>()
            .unwrap()
            .list("file-test".into())
            .await
            .unwrap();
        assert!(
            tasks
                .iter()
                .any(|task| task.title == if name == "copyFile" { "COPY" } else { "MOVE" })
        );
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn unsupported_trash_actions_report_the_capability_without_requiring_a_storage_grant() {
    let (_dir, schema) = fixture("[]");
    for action in ["trashMediaItems", "restoreMediaItems"] {
        let response = schema
            .execute(format!(
                r#"mutation {{ {action}(type:IMAGE, query:"all:true") {{ affectedCount }} }}"#
            ))
            .await;
        assert_eq!(response.errors.len(), 1);
        assert_eq!(
            response.errors[0].message,
            "unsupported capability MEDIA_TRASH"
        );
    }
}

/// Moving media is a storage write; the trash actions only move rows.
#[tokio::test]
async fn moving_media_is_gated() {
    let (_dir, schema) = fixture_with("[]", |method, _| match method {
        "systemPermissionFacts" => json!({ "granted": { "WRITE_EXTERNAL_STORAGE": true } }),
        other => panic!("the platform must not be asked: {other}"),
    });
    let response = schema
        .execute(r#"mutation { moveMediaItems(type:IMAGE, query:"text:x", destDir:"/d") { affectedCount } }"#)
        .await;
    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "no_permission");
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn moving_media_uses_the_shared_native_commit_and_preserves_destination_collisions() {
    let (dir, schema) = fixture(GRANTED);
    let rows = seed_media(&schema, plain_rs::enums::DataType::Doc, 2, false, "pdf");
    let destination = dir.path().canonicalize().unwrap().join("target");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("x1.pdf"), b"existing").unwrap();
    let response = schema
        .execute(format!(
            r#"mutation {{moveMediaItems(type:DOC,query:"all:true",destDir:{}){{affectedCount}}}}"#,
            json!(destination)
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["moveMediaItems"]["affectedCount"],
        2
    );
    assert_eq!(
        std::fs::read(destination.join("x1.pdf")).unwrap(),
        b"existing"
    );
    assert_eq!(
        std::fs::read(destination.join("x1 (1).pdf")).unwrap(),
        b"fixture"
    );
    assert_eq!(
        std::fs::read(destination.join("x2.pdf")).unwrap(),
        b"fixture"
    );
    for row in rows {
        assert!(!std::path::Path::new(&row.path).exists());
    }
}

/// A blank query matches every row of the kind. For a bulk delete that is
/// the difference between a mistake and data loss, so it is refused before
/// the platform is asked.
#[tokio::test]
async fn a_blank_media_query_is_refused_before_the_platform() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _| match method {
        "systemPermissionFacts" => json!({ "granted": { "WRITE_EXTERNAL_STORAGE": true } }),
        other => panic!("the platform must not be asked: {other}"),
    });
    for name in [
        "deleteMediaItems",
        "trashMediaItems",
        "restoreMediaItems",
        "moveMediaItems",
    ] {
        let document = if name == "moveMediaItems" {
            format!(
                r#"mutation {{ {name}(type:IMAGE, query:"", destDir:"/d") {{ affectedCount }} }}"#
            )
        } else {
            format!(r#"mutation {{ {name}(type:IMAGE, query:"   ") {{ affectedCount }} }}"#)
        };
        let response = schema.execute(&document).await;
        assert_eq!(response.errors.len(), 1, "{name}");
        assert_eq!(
            response.errors[0].message, "explicit query required",
            "{name}"
        );
    }
}

fn upload_fixture() -> (tempfile::TempDir, PublicSchema, std::path::PathBuf) {
    let (dir, schema) = fixture_with(GRANTED, |method, _| match method {
        "systemPermissionFacts" => json!({"granted":{"WRITE_EXTERNAL_STORAGE":true}}),
        other => panic!("Unexpected platform primitive: {other}"),
    });
    let base = dir.path().join("uploads");
    (dir, schema, base)
}
async fn merge_status(schema: &PublicSchema, id: &str) -> Value {
    let response = schema
        .execute(format!(
            r#"{{mergeStatus(fileId:"{id}"){{status value mergedSize error}}}}"#
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()["mergeStatus"].clone()
}
async fn wait_merge(schema: &PublicSchema, id: &str) -> Value {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let status = merge_status(schema, id).await;
        if matches!(status["status"].as_str(), Some("DONE" | "FAILED")) {
            return status;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Merge did not complete: {status}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}
#[tokio::test]
async fn graphql_uploads_list_merge_and_recover_the_same_rust_job() {
    let (temp, schema, base) = upload_fixture();
    let chunks = base.join("abc");
    tokio::fs::create_dir_all(&chunks).await.unwrap();
    tokio::fs::write(chunks.join("chunk_1"), b"def")
        .await
        .unwrap();
    tokio::fs::write(chunks.join("chunk_0"), b"abc")
        .await
        .unwrap();
    let response = schema.execute(r#"{uploadedChunks(fileId:"abc")}"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["uploadedChunks"],
        json!(["0:3", "1:3"])
    );
    assert_eq!(merge_status(&schema, "abc").await["status"], "NONE");
    let target = temp.path().join("result.bin");
    let response=schema.execute(format!(r#"mutation {{mergeChunks(fileId:"abc",totalChunks:2,path:"{}",replace:true,totalSize:6){{status}}}}"#,target.display())).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["mergeChunks"]["status"],
        "STARTED"
    );
    let status = wait_merge(&schema, "abc").await;
    assert_eq!(status["status"], "DONE");
    assert_eq!(status["value"], "result.bin");
    assert_eq!(status["mergedSize"], 6);
    assert_eq!(status["error"], Value::Null);
    assert_eq!(tokio::fs::read(&target).await.unwrap(), b"abcdef");
    let response=schema.execute(format!(r#"mutation {{mergeChunks(fileId:"abc",totalChunks:2,path:"{}",replace:true,totalSize:6){{status}}}}"#,target.display())).await;
    assert!(response.errors.is_empty());
    assert_eq!(
        response.data.into_json().unwrap()["mergeChunks"]["status"],
        "DONE"
    );
}
#[tokio::test]
async fn graphql_failed_merge_keeps_error_and_no_value() {
    let (temp, schema, base) = upload_fixture();
    let chunks = base.join("abc");
    tokio::fs::create_dir_all(&chunks).await.unwrap();
    tokio::fs::write(chunks.join("chunk_0"), b"abc")
        .await
        .unwrap();
    let response=schema.execute(format!(r#"mutation {{mergeChunks(fileId:"abc",totalChunks:2,path:"{}",replace:false,totalSize:6){{status}}}}"#,temp.path().join("result").display())).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let status = wait_merge(&schema, "abc").await;
    assert_eq!(status["status"], "FAILED");
    assert!(
        status["error"]
            .as_str()
            .unwrap()
            .contains("Missing chunk 1")
    );
    assert_eq!(status["value"], Value::Null);
    assert!(chunks.exists());
}

#[tokio::test]
async fn file_uri_exports_replace_complete_bytes_and_preserve_existing_files_on_failed_merge() {
    for complete in [true, false] {
        let (directory, schema, base) = upload_fixture();
        let target = directory.path().join("feed export + \u{6d4b}\u{8bd5}.opml");
        let neighbor = directory.path().join("unselected.opml");
        std::fs::write(&target, b"previous export").unwrap();
        std::fs::write(&neighbor, b"unselected bytes").unwrap();
        let uri = reqwest::Url::from_file_path(&target).unwrap().to_string();
        assert!(uri.contains("%20"));
        let chunks = base.join("export");
        std::fs::create_dir_all(&chunks).unwrap();
        std::fs::write(chunks.join("chunk_0"), b"<opml>").unwrap();
        if complete {
            std::fs::write(chunks.join("chunk_1"), b"</opml>").unwrap();
        }
        let response = schema.execute(async_graphql::Request::new(
            "mutation($path:String!){mergeChunks(fileId:\"export\",totalChunks:2,path:$path,replace:true,totalSize:13){status}}",
        ).variables(async_graphql::Variables::from_json(json!({"path":uri})))).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let result = wait_merge(&schema, "export").await;
        if complete {
            assert_eq!(result["status"], "DONE");
            assert_eq!(std::fs::read(&target).unwrap(), b"<opml></opml>");
        } else {
            assert_eq!(result["status"], "FAILED");
            assert_eq!(std::fs::read(&target).unwrap(), b"previous export");
        }
        assert_eq!(std::fs::read(neighbor).unwrap(), b"unselected bytes");
    }
}
#[tokio::test]
async fn graphql_app_file_merge_imports_into_the_rust_store() {
    let (_temp, schema, base) = upload_fixture();
    let chunks = base.join("abc");
    tokio::fs::create_dir_all(&chunks).await.unwrap();
    tokio::fs::write(chunks.join("chunk_0"), b"image")
        .await
        .unwrap();
    let response=schema.execute(r#"mutation {mergeAppFileChunks(fileId:"abc",totalChunks:1,fileName:"photo.jpg",totalSize:5){status}}"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let status = wait_merge(&schema, "abc").await;
    assert_eq!(status["status"], "DONE");
    assert!(!status["value"].as_str().unwrap().is_empty());
    assert_eq!(status["mergedSize"], 5);
    assert!(!chunks.exists());
}

#[test]
fn merge_status_reads_back_the_names_the_contract_uses() {
    assert_eq!(MergeTaskStatus::parse("NONE"), MergeTaskStatus::None);
    assert_eq!(MergeTaskStatus::parse("STARTED"), MergeTaskStatus::Started);
    assert_eq!(MergeTaskStatus::parse("MERGING"), MergeTaskStatus::Merging);
    assert_eq!(MergeTaskStatus::parse("DONE"), MergeTaskStatus::Done);
    assert_eq!(MergeTaskStatus::parse("FAILED"), MergeTaskStatus::Failed);
    // An unrecognised state is "nothing happened yet" rather than a state
    // the client has never heard of.
    assert_eq!(MergeTaskStatus::parse("WAT"), MergeTaskStatus::None);
}

/// The predicate is worth nothing if the actions do not ask it, so this drives
/// the real mutation. Audio contributes no clause of its own, so `type:1` —
/// what the Apps view leaves behind when the user navigates to a media one —
/// used to resolve to the whole library.
#[tokio::test]
async fn a_media_action_refuses_a_query_that_selects_nothing_and_all_true_still_goes_through() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _| match method {
        "systemPermissionFacts" => json!({ "granted": { "WRITE_EXTERNAL_STORAGE": true } }),
        "systemMediaIdsFacts" => json!([]),
        other => panic!("unexpected host call {other}"),
    });

    for mutation in [
        r#"mutation { deleteMediaItems(type: AUDIO, query: "type:1") { affectedCount } }"#,
        r#"mutation { trashMediaItems(type: AUDIO, query: "type:1") { affectedCount } }"#,
        r#"mutation { restoreMediaItems(type: AUDIO, query: "type:1") { affectedCount } }"#,
        r#"mutation { moveMediaItems(type: AUDIO, query: "type:1", destDir: "/sdcard/x") { affectedCount } }"#,
        r#"mutation { trashMediaItems(type: IMAGE, query: "show_hidden:false") { affectedCount } }"#,
    ] {
        let response = schema.execute(mutation).await;
        assert_eq!(response.errors.len(), 1, "{mutation} was not refused");
        assert!(
            response.errors[0].message.contains("selects no"),
            "{mutation} failed for the wrong reason: {}",
            response.errors[0].message
        );
    }

    // `all:true` builds no clause either, and says so — refusing it would
    // leave no way to target everything on purpose.
    let response = schema
        .execute(
            r#"mutation { deleteMediaItems(type: AUDIO, query: "all:true") { affectedCount } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(format!("{:?}", response.data).contains("affectedCount"));
}

#[tokio::test]
async fn media_batches_commit_successes_and_notify_only_confirmed_ids() {
    let (_dir, schema) = fixture(GRANTED);
    let rows = seed_media(&schema, plain_rs::enums::DataType::Video, 501, false, "mp4");
    let selected = crate::services::files::catalog::MediaCatalog::new(
        database(&schema).clone(),
        schema.data::<Arc<Host>>().unwrap().clone(),
    )
    .ids(plain_rs::enums::DataType::Video, "all:true", false)
    .await
    .unwrap();
    let casualty = selected[6].clone();
    let failed = rows.iter().find(|row| row.id == casualty).unwrap();
    std::fs::remove_file(&failed.path).unwrap();
    let db = schema.data::<Arc<Db>>().unwrap();
    let tag =
        plain_rs::tags::create_tag(db, plain_rs::enums::DataType::Video.kind(), "keep failed")
            .unwrap();
    plain_rs::tags::add_relations(db, &[(tag.id.clone(), casualty.clone())]).unwrap();
    let mut events = schema
        .data::<tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>>()
        .unwrap()
        .subscribe();
    let response = schema
        .execute(r#"mutation {deleteMediaItems(type:VIDEO,query:"all:true"){affectedCount}}"#)
        .await;
    assert_eq!(response.errors.len(), 1);
    assert!(
        response.errors[0]
            .message
            .contains("1 media actions failed (500 completed)")
    );

    assert_eq!(
        plain_rs::tags::keys_for_tag(db, &tag.id).unwrap(),
        vec![casualty.clone()]
    );
    let mut refreshes = 0;
    while let Ok(event) = events.try_recv() {
        if event.event_type == "CONTENT_CHANGED" {
            assert!(!event.is_internal());
            refreshes += 1;
        }
    }
    assert_eq!(refreshes, 2);
    let remaining = db
        .with_conn(|connection| {
            connection
                .prepare("SELECT id FROM file_catalog WHERE data_type=2")?
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap();
    assert_eq!(remaining, vec![casualty]);
}

#[test]
fn merge_results_preserve_status_output_size_and_failure() {
    assert_eq!(
        merge_task(&json!({"status":"NONE"})).status,
        MergeTaskStatus::None
    );
    assert_eq!(
        merge_task(&json!({"status":"MERGING"})).status,
        MergeTaskStatus::Merging
    );
    let done = merge_task(&json!({"status":"DONE","value":"a.jpg","mergedSize":42}));
    assert_eq!(done.status, MergeTaskStatus::Done);
    assert_eq!(done.value.as_deref(), Some("a.jpg"));
    assert_eq!(done.merged_size.map(|v| v.0), Some(42));
    assert!(done.error.is_none());
    let failed = merge_task(&json!({"status":"FAILED","error":"boom"}));
    assert_eq!(failed.status, MergeTaskStatus::Failed);
    assert_eq!(failed.error.as_deref(), Some("boom"));
    assert!(failed.value.is_none());
}

#[tokio::test]
async fn main_upload_resume_query_observes_only_completed_native_byte_streams() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let executor = core.native_graphql().unwrap();
    let query = r#"{uploadedChunks(fileId:"stream-fixture")}"#;
    let uploads = core.native_uploads().unwrap();
    let mut pending = uploads.open_chunk("stream-fixture", 0, 3).await.unwrap();
    pending.write(b"abc").await.unwrap();
    let response = executor.execute(async_graphql::Request::new(query)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["uploadedChunks"],
        json!([])
    );
    pending.finish().await.unwrap();
    let mut aborted = uploads.open_chunk("stream-fixture", 1, 3).await.unwrap();
    aborted.write(b"x").await.unwrap();
    drop(aborted);
    let response = executor.execute(async_graphql::Request::new(query)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["uploadedChunks"],
        json!(["0:3"])
    );
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
    assert!(
        !executor
            .execute(async_graphql::Request::new(query))
            .await
            .errors
            .is_empty()
    );
}

#[tokio::test]
async fn existing_media_delete_uses_shared_batches_and_refuses_blank_queries_before_effects() {
    let (_directory, schema) = fixture(GRANTED);
    let rows = seed_media(&schema, plain_rs::enums::DataType::Video, 501, false, "mp4");
    let ids = rows.iter().map(|row| row.id.clone()).collect::<Vec<_>>();
    let document =
        r#"mutation($query:String!){deleteMediaItems(type:VIDEO,query:$query){affectedCount}}"#;
    for query in ["", " ", "\n\t"] {
        let response = schema
            .execute(
                async_graphql::Request::new(document)
                    .variables(async_graphql::Variables::from_json(json!({"query":query}))),
            )
            .await;
        assert_eq!(response.errors.len(), 1, "blank query must be rejected");
    }
    assert!(
        rows.iter()
            .all(|row| std::path::Path::new(&row.path).exists())
    );
    for index in [6, 500] {
        std::fs::remove_file(&rows[index].path).unwrap();
    }
    let mut requested = ids.clone();
    requested.push(ids[0].clone());
    let response = schema
        .execute(async_graphql::Request::new(document).variables(
            async_graphql::Variables::from_json(
                json!({"query":format!("ids:{} trash:false",requested.join(","))}),
            ),
        ))
        .await;
    assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
    assert!(
        response.errors[0]
            .message
            .contains("2 media actions failed (499 completed)")
    );
    let db = database(&schema);
    let remaining = db
        .with_conn(|connection| {
            connection.query_row(
                "SELECT COUNT(*) FROM file_catalog WHERE data_type=2",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .unwrap();
    assert_eq!(remaining, 2);
    assert!(
        rows.iter()
            .all(|row| !std::path::Path::new(&row.path).exists())
    );
}

#[test]
fn file_id_selection_preserves_opaque_values_without_adding_filters() {
    let fields = plain_rs::utils::search_dsl::parse(
        r#"ids:==opaque,17\ all:true,\"quoted\"\\path,媒体：42 trash:false"#,
    );
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "ids");
    assert_eq!(fields[0].op, "=");
    assert_eq!(
        fields[0].value,
        "=opaque,17 all:true,\"quoted\"\\path,媒体：42"
    );
    assert_eq!(fields[1].name, "trash");
    assert_eq!(fields[1].value, "false");
}
