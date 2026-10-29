use super::*;
use crate::assembly::server::ContentServer;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;
async fn call(port: u16, token: &str, doc: String) -> Value {
    let text = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/graphql"))
        .bearer_auth(token)
        .header("content-type", "application/json")
        .body(json!({"query":doc}).to_string())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    serde_json::from_str(&text).unwrap()
}
fn find_task(response: &Value, id: &str) -> Value {
    assert!(response.get("errors").is_none(), "{response}");
    response["data"]["fileTasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == id)
        .unwrap()
        .clone()
}
#[tokio::test]
async fn main_tasks_track_resolved_paths_and_gate_sdk_execution_on_platform_authorization() {
    let temp = tempfile::tempdir().unwrap();
    let token = plain_rs::utils::base64::base64_encode(&[12; 32]);
    let db_path = temp.path().join("core/tasks.db");
    let server = ContentServer::start_with_config(
        db_path.parent().unwrap(),
        &token,
        Arc::new(plain_rs::prefs::Prefs::load(&temp.path().join("prefs.json")).unwrap()),
        crate::host::startup::StartupConfig {
            system_media_catalog: true,
            ..crate::platform::startup::config()
        },
    )
    .unwrap();
    server.runtime_state().file_platform.use_sdk_for_test();
    let mut socket = crate::host::events::TestHost::new(&server);
    let revoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let denied = revoked.clone();
    let scanned = Arc::new(Mutex::new(Vec::<String>::new()));
    let records = scanned.clone();
    let worker = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            let request: Value = serde_json::from_str(&text).unwrap();
            let reply = if request["method"] == "systemCapabilities" {
                json!({"id":request["id"],"result":[]})
            } else if denied.load(std::sync::atomic::Ordering::SeqCst) {
                json!({"id":request["id"],"error":"synthetic permission revoked"})
            } else {
                match request["method"].as_str().unwrap() {
                    "systemPermissionFacts" => {}
                    "fileTaskScan" => records.lock().unwrap().extend(
                        request["params"]["paths"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|value| value.as_str().unwrap().to_owned()),
                    ),
                    method => panic!("unexpected method {method}"),
                }
                json!({"id":request["id"],"result":if request["method"] == "systemPermissionFacts" {json!({"granted":{"WRITE_EXTERNAL_STORAGE":true}})} else {json!(true)}})
            };
            socket
                .send(Message::Text(reply.to_string().into()))
                .await
                .unwrap();
        }
    });
    let src = temp.path().join("source.txt");
    let dst = temp.path().join("target.txt");
    std::fs::write(&src, b"synthetic").unwrap();
    std::fs::write(&dst, b"existing").unwrap();
    let document = format!(
        "mutation {{ createFileTask(type:COPY,title:\"copy\",ops:[{{src:{},dst:{},overwrite:false}}]) {{ id status }} }}",
        json!(src.to_str().unwrap()),
        json!(dst.to_str().unwrap())
    );
    let queued = call(server.port, &token, document.clone()).await;
    assert!(queued.get("errors").is_none(), "{queued}");
    let id = queued["data"]["createFileTask"]["id"].as_str().unwrap();
    let done = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let result = call(server.port,&token,"query { fileTasks(query:\"\",offset:0,limit:2147483647) { id status error doneBytes completedOps { src dst } } }".to_owned()).await;
            assert!(result.get("errors").is_none(),"{result}");
            let task = find_task(&result, id);
            if task["status"] == "DONE" || task["status"] == "ERROR" { break task; }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    assert_eq!(done["status"], "DONE");
    let resolved = temp.path().join("target (1).txt");
    assert_eq!(done["completedOps"][0]["dst"], resolved.to_str().unwrap());
    assert!(
        scanned
            .lock()
            .unwrap()
            .contains(&resolved.to_str().unwrap().to_owned())
    );
    assert_eq!(std::fs::read(&resolved).unwrap(), b"synthetic");
    assert_eq!(std::fs::read(&dst).unwrap(), b"existing");
    let foreign = server
        .runtime_state()
        .public
        .execute(
            async_graphql::Request::new(
                "{ fileTasks(query:\"\",offset:0,limit:2147483647) { id } }",
            )
            .data("foreign".to_owned()),
        )
        .await;
    assert!(foreign.errors.is_empty(), "{:?}", foreign.errors);
    assert_eq!(foreign.data.into_json().unwrap()["fileTasks"], json!([]));
    revoked.store(true, std::sync::atomic::Ordering::SeqCst);
    let denied = call(server.port, &token, document).await;
    assert!(denied.get("errors").is_some());
    assert!(!temp.path().join("target (2).txt").exists());
    let rows = server
        .runtime_state()
        .files
        .list(server.runtime_state().native_client_id().unwrap())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, FileTaskStatus::Done);
    let removed = call(
        server.port,
        &token,
        format!("mutation {{ removeFileTask(id:{}) }}", json!(id)),
    )
    .await;
    assert!(removed.get("errors").is_some());
    assert_eq!(
        server
            .runtime_state()
            .files
            .list(server.runtime_state().native_client_id().unwrap())
            .await
            .unwrap()
            .len(),
        1
    );
    worker.abort();
    let _ = worker.await;
    server.shutdown().await;
}

async fn media_move_case(missing_destination: bool) {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let source = base.join("source");
    std::fs::create_dir(&source).unwrap();
    for name in ["audio", "video", "image", "document"] {
        std::fs::write(source.join(name), b"fixture").unwrap();
    }
    let destination = base.join("target");
    std::fs::write(&destination, b"existing").unwrap();
    let db_path = base.join("core/db");
    let token = plain_rs::utils::base64::base64_encode(&[16; 32]);
    let server = ContentServer::start_with_config(
        db_path.parent().unwrap(),
        &token,
        Arc::new(plain_rs::prefs::Prefs::load(&base.join("prefs")).unwrap()),
        crate::host::startup::StartupConfig {
            system_media_catalog: true,
            ..crate::platform::startup::config()
        },
    )
    .unwrap();
    let db = server.runtime_state().db.clone();
    let audio_path = source.join("audio").to_str().unwrap().to_owned();
    db.with_conn(|c| -> rusqlite::Result<()> {
        for (kind, id, tag) in [(1,"1","audio"),(2,"2","video"),(3,"3","image"),(24,"4","document")] {
            c.execute("INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES(?1,?2,?3,'first')",rusqlite::params![tag,id,kind])?;
        }
        c.execute("INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('file',?1,22,'first')",[source.join("document").to_str().unwrap()])?;
        c.execute_batch("INSERT INTO media_play_history(media_type,file_id,position_ms,play_count,played_at) VALUES(2,'2',4000000001,0,'first');")?;
        c.execute("INSERT INTO audio_queue_items(file_id,sort_order) SELECT id,0 FROM file_catalog WHERE path=?1",[&audio_path])?;

        Ok(())
    }).unwrap();
    let duration_cache =
        plain_rs::media::duration_cache::DurationCache::new(db_path.parent().unwrap());
    duration_cache
        .save(plain_rs::media::duration_cache::Duration {
            media_type: "audio".into(),
            file_id: "1".into(),
            path: audio_path.clone(),
            size_bytes: 7,
            modified_at: "2026-01-01T00:00:00.000Z".into(),
            duration_ms: 5_000_000_001,
            updated_at: String::new(),
        })
        .unwrap();
    db.use_platform_audio_catalog().unwrap();
    db.cache_platform_audio(
        &[plain_rs::db::audio_catalog::AudioMedia {
            file_id: Some("1".into()),
            aliases: Vec::new(),
            track: plain_rs::media::audio::queue::AudioTrack::from_path_stem(&audio_path),
        }],
        false,
    )
    .unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO audio_queue_items(file_id,sort_order) VALUES('1',0)",
            [],
        )
    })
    .unwrap();
    plain_rs::media::audio::queue::save_audio_current(&db, &audio_path).unwrap();
    plain_rs::media::audio::playback::seek(&db, 100).unwrap();
    server.runtime_state().file_platform.use_sdk_for_test();
    let mut socket = crate::host::events::TestHost::new(&server);
    let source_root = source.clone();
    let clear_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let clear = clear_count.clone();
    let unavailable = Arc::new(std::sync::atomic::AtomicBool::new(missing_destination));
    let missing = unavailable.clone();
    let worker = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            let request: Value = serde_json::from_str(&text).unwrap();
            let params = &request["params"];
            let result = match request["method"].as_str().unwrap() {
                "fileTaskScan" => json!(true),
                "systemCapabilities" => json!([]),
                "systemPermissionFacts" => json!({"granted":{"WRITE_EXTERNAL_STORAGE":true}}),
                "fileTaskMediaSnapshot" => {
                    let rows = params["paths"].as_array().unwrap().iter().filter_map(|path| {
                        let path = path.as_str().unwrap();
                        let old = PathBuf::from(path).starts_with(&source_root);
                        if missing.load(std::sync::atomic::Ordering::SeqCst) && !old { return None }
                        let (kind,id) = match PathBuf::from(path).file_name().unwrap().to_str().unwrap() {
                            "audio" => (1,1), "video" => (2,2), "image" => (3,3), "document" => (24,4), _ => panic!("unexpected path"),
                        };
                        assert!(PathBuf::from(path).is_file());
                        Some(json!({"providerType":match kind {1=>2,2=>3,3=>1,_=>0},"id":(if old{id}else{id+10}).to_string(),"mimeType":if kind==24 {"text/plain"} else {"application/octet-stream"},"path":path}))
                    }).collect::<Vec<_>>();
                    json!(rows)
                }
                "audioLibraryResolve" => {
                    let mut rows = Vec::new();
                    for id in params["ids"].as_array().unwrap() {
                        let id = id.as_str().unwrap();
                        let path = match id {
                            "1" => source_root.join("audio"),
                            "11" => source_root
                                .parent()
                                .unwrap()
                                .join("target (1)")
                                .join("audio"),
                            _ => continue,
                        };
                        if path.is_file() {
                            rows.push(json!({"mediaId":id,"path":path.to_str().unwrap(),"title":"fixture","artist":"","albumId":"","durationMs":5000000001_i64}));
                        }
                    }
                    json!(rows)
                }
                "audioEngineCommand" => {
                    assert_eq!(params["action"], "CLEAR");
                    clear.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    json!({"path":params["playback"]["path"],"positionMs":0,"revision":params["playback"]["revision"],"loaded":false})
                }
                method => panic!("unexpected method {method}"),
            };
            socket
                .send(Message::Text(
                    json!({"id":request["id"],"result":result})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
        }
    });
    let queued = call(server.port,&token,format!("mutation {{ createFileTask(type:MOVE,title:\"move\",ops:[{{src:{},dst:{},overwrite:false}}]) {{ id }} }}",json!(source.to_str().unwrap()),json!(destination.to_str().unwrap()))).await;
    assert!(queued.get("errors").is_none(), "{queued}");
    let id = queued["data"]["createFileTask"]["id"].as_str().unwrap();
    let task = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let row = call(
                server.port,
                &token,
                "query { fileTasks(query:\"\",offset:0,limit:2147483647) { id status error completedOps { src dst } } }".to_owned(),
            )
            .await;
            let task = find_task(&row, id);
            if task["status"] == "DONE" || task["status"] == "ERROR" {
                break task;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let moved = base.join("target (1)");
    assert!(!source.exists());
    assert!(duration_cache.all().unwrap().is_empty());
    assert!(moved.join("audio").is_file());
    assert_eq!(task["completedOps"][0]["dst"], moved.to_str().unwrap());
    assert_eq!(std::fs::read(&destination).unwrap(), b"existing");
    if missing_destination {
        assert_eq!(task["status"], "ERROR");
        assert!(
            task["error"]
                .as_str()
                .unwrap()
                .contains("moved media identity missing")
        );
        assert_eq!(clear_count.load(std::sync::atomic::Ordering::SeqCst), 0);
        db.with_conn(|c| {
            c.query_row(
                "SELECT target_key FROM tag_relations WHERE tag_id='audio'",
                [],
                |r| r.get::<_, String>(0),
            )
        })
        .map(|id| assert_eq!(id, "1"))
        .unwrap();
        unavailable.store(false, std::sync::atomic::Ordering::SeqCst);
        let row = call(
            server.port,
            &token,
            "query { fileTasks(query:\"\",offset:0,limit:2147483647) { id status } }".to_owned(),
        )
        .await;
        assert_eq!(find_task(&row, id)["status"], "ERROR");
        assert_eq!(clear_count.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(!base.join("target (2)").exists());
    } else {
        assert_eq!(task["status"], "DONE", "{task}");
        assert_eq!(clear_count.load(std::sync::atomic::Ordering::SeqCst), 1);
        db.with_conn(|c| -> rusqlite::Result<()> {
            for (tag, id) in [
                ("audio", "11"),
                ("video", "12"),
                ("image", "13"),
                ("document", "14"),
            ] {
                assert_eq!(
                    c.query_row(
                        "SELECT target_key FROM tag_relations WHERE tag_id=?1",
                        [tag],
                        |r| r.get::<_, String>(0)
                    )?,
                    id
                );
            }
            assert_eq!(
                c.query_row(
                    "SELECT target_key FROM tag_relations WHERE tag_id='file'",
                    [],
                    |r| r.get::<_, String>(0)
                )?,
                moved.join("document").to_str().unwrap()
            );
            assert_eq!(
                c.query_row(
                    "SELECT position_ms FROM media_play_history WHERE file_id='12'",
                    [],
                    |r| r.get::<_, i64>(0)
                )?,
                4000000001
            );
            assert_eq!(
                c.query_row("SELECT file_id FROM audio_queue_items", [], |r| r
                    .get::<_, String>(0))?,
                "11"
            );
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(
        plain_rs::media::audio::playback::snapshot(&db)
            .unwrap()
            .path,
        if missing_destination {
            audio_path
        } else {
            moved.join("audio").to_str().unwrap().to_owned()
        }
    );
    worker.abort();
    let _ = worker.await;
    server.shutdown().await;
}
#[tokio::test]
async fn directory_move_rebinds_actual_native_ids_and_stops_stale_playback() {
    media_move_case(false).await;
}
#[tokio::test]
async fn missing_native_destination_keeps_metadata_and_reports_actual_completed_move() {
    media_move_case(true).await;
}

#[tokio::test(start_paused = true)]
async fn synchronous_completion_waits_for_notification_without_periodic_reads() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let notification = Arc::new(tokio::sync::Notify::new());
    let ready = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    let (started, pending) = tokio::sync::oneshot::channel();
    let worker = {
        let notification = notification.clone();
        let ready = ready.clone();
        let reads = reads.clone();
        tokio::spawn(async move {
            let mut started = Some(started);
            wait_for_completion(&notification, || {
                reads.fetch_add(1, Ordering::SeqCst);
                if let Some(started) = started.take() {
                    started.send(()).unwrap();
                }
                Ok(ready.load(Ordering::SeqCst).then_some(7))
            })
            .await
        })
    };
    pending.await.unwrap();
    tokio::time::advance(std::time::Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    ready.store(true, Ordering::SeqCst);
    notification.notify_waiters();
    assert_eq!(worker.await.unwrap().unwrap(), 7);
    assert_eq!(reads.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn completion_between_state_read_and_wait_is_not_lost() {
    let notification = tokio::sync::Notify::new();
    let mut reads = 0;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        wait_for_completion(&notification, || {
            reads += 1;
            if reads == 1 {
                notification.notify_waiters();
                Ok(None)
            } else {
                Ok(Some("done"))
            }
        }),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result, "done");
    assert_eq!(reads, 2);
}

#[tokio::test]
async fn main_create_task_can_wait_for_completion_and_returns_partial_failure_receipts() {
    let temp = tempfile::tempdir().unwrap();
    let server = ContentServer::open(&temp.path().join("core")).unwrap();
    let state = server.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let source = temp.path().join("source.txt");
    std::fs::write(&source, b"source").unwrap();
    let copied = temp.path().join("copied.txt");
    let native = server.native_graphql().unwrap();
    let document = r#"mutation($ops:[FileTaskOpInput!]!){createFileTask(type:COPY,title:"Paste selection",ops:$ops,waitForCompletion:true){id title status error doneBytes completedOps{src dst}}}"#;
    let response = native
        .execute(async_graphql::Request::new(document).variables(
            async_graphql::Variables::from_json(
                json!({"ops":[{"src":source,"dst":copied,"overwrite":false}]}),
            ),
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let result = response.data.into_json().unwrap();
    let task = &result["createFileTask"];
    assert_eq!(task["status"], "DONE");
    assert_eq!(task["title"], "Paste selection");
    assert_eq!(task["doneBytes"], 6);
    assert_eq!(task["completedOps"], json!([{"src":source,"dst":copied}]));
    assert_eq!(std::fs::read(&copied).unwrap(), b"source");
    let tree = temp.path().join("tree");
    std::fs::create_dir(&tree).unwrap();
    std::fs::write(tree.join("file.txt"), b"tree").unwrap();
    let partial = temp.path().join("partial.txt");
    let response = native.execute(async_graphql::Request::new(document).variables(async_graphql::Variables::from_json(json!({"ops":[{"src":source,"dst":partial,"overwrite":false},{"src":tree,"dst":tree.join("child"),"overwrite":false}]})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let result = response.data.into_json().unwrap();
    let task = &result["createFileTask"];
    assert_eq!(task["status"], "ERROR");
    assert!(!task["error"].as_str().unwrap().is_empty());
    assert_eq!(task["completedOps"], json!([{"src":source,"dst":partial}]));
    assert_eq!(std::fs::read(&partial).unwrap(), b"source");
    assert_eq!(std::fs::read(tree.join("file.txt")).unwrap(), b"tree");
    assert!(requests.try_recv().is_err());
    server.shutdown().await;
}

#[tokio::test]
async fn destructive_path_batches_validate_every_target_before_touching_any_file_or_host() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state.file_platform.use_sdk_for_test();
    let (_generation, mut requests) = state.host.connect();
    let path = directory.path().join("preserved.txt");
    std::fs::write(&path, b"preserve me").unwrap();
    for invalid in ["", " \t\n", "/", "."] {
        let paths = vec![path.to_string_lossy().into_owned(), invalid.into()];
        assert!(state.files.trash_paths(paths.clone()).await.is_err());
        assert!(state.files.delete_many(paths).await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"preserve me");
        assert!(requests.try_recv().is_err());
    }
    for suffix in [".", ".."] {
        let invalid = directory.path().join(suffix).to_string_lossy().into_owned();
        let paths = vec![path.to_string_lossy().into_owned(), invalid];
        assert!(state.files.trash_paths(paths.clone()).await.is_err());
        assert!(state.files.delete_many(paths).await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"preserve me");
        assert!(requests.try_recv().is_err());
    }
    assert_eq!(state.files.trash_paths(vec![]).await.unwrap(), 0);
    assert_eq!(state.files.delete_many(vec![]).await.unwrap(), 0);
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

async fn whole_selection_preflight_case(sdk: bool) {
    for kind in [FileTaskType::Copy, FileTaskType::Move] {
        let directory = tempfile::tempdir().unwrap();
        let core = ContentServer::open(&directory.path().join("core")).unwrap();
        let state = core.runtime_state();
        if sdk {
            state.file_platform.use_sdk_for_test();
        }
        let (generation, mut requests) = state.host.connect();
        let host = state.host.clone();
        let receipts = Arc::new(Mutex::new(Vec::<String>::new()));
        let observed = receipts.clone();
        let worker = tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                let method = request["method"].as_str().unwrap().to_owned();
                observed.lock().unwrap().push(method.clone());
                host.reply(generation, json!({"id":request["id"],"result":if method == "systemPermissionFacts" { json!({"granted":{"WRITE_EXTERNAL_STORAGE":true}}) } else { json!(true) }})).unwrap();
            }
        });
        let source = directory.path().join("selected-first.bin");
        let destination = directory.path().join("first-result.bin");
        let missing = directory.path().join("selected-second-missing.bin");
        let missing_destination = directory.path().join("second-result.bin");
        std::fs::write(&source, b"first selected exact bytes").unwrap();
        let client = state.native_client_id().unwrap();
        let before = state.files.list(client.clone()).await.unwrap();
        let result = state
            .files
            .create(
                &client,
                kind,
                "whole-selection",
                vec![
                    FileTaskOp {
                        src: source.to_string_lossy().into_owned(),
                        dst: destination.to_string_lossy().into_owned(),
                        overwrite: false,
                    },
                    FileTaskOp {
                        src: missing.to_string_lossy().into_owned(),
                        dst: missing_destination.to_string_lossy().into_owned(),
                        overwrite: false,
                    },
                ],
            )
            .await;
        assert!(
            result.is_err(),
            "a missing selected source was admitted: {result:?}"
        );
        assert_eq!(
            result
                .unwrap_err()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(
            serde_json::to_value(state.files.list(client).await.unwrap()).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        assert_eq!(
            std::fs::read(&source).unwrap(),
            b"first selected exact bytes"
        );
        assert!(!destination.exists());
        assert!(!missing_destination.exists());
        let calls = receipts.lock().unwrap().clone();
        if sdk {
            assert_eq!(calls, vec!["systemPermissionFacts"]);
        } else {
            assert!(calls.is_empty());
        }
        worker.abort();
        let _ = worker.await;
        core.shutdown().await;
    }
}

#[tokio::test]
async fn sdk_admission_checks_every_selected_source_before_recording_a_task_or_running_the_first_operation()
 {
    whole_selection_preflight_case(true).await;
}

#[tokio::test]
async fn native_admission_keeps_the_same_whole_selection_contract_for_copy_and_move() {
    whole_selection_preflight_case(false).await;
}
