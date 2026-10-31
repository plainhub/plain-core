use super::*;
use std::sync::Mutex as StdMutex;

struct Fixture {
    directory: tempfile::TempDir,
    service: Arc<Mutations>,
    host: Arc<Host>,
    generation: u64,
    worker: tokio::task::JoinHandle<()>,
    calls: Arc<StdMutex<Vec<String>>>,
}

impl Fixture {
    fn new(
        ids: Vec<String>,
        delete: impl Fn() -> Option<Result<Value, String>> + Send + 'static,
    ) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open(&directory.path().join("data.db")).unwrap());
        let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
        let host = Arc::new(Host::default());
        let service = Mutations::new(db, host.clone(), prefs);
        let (generation, mut requests) = host.connect();
        let replies = host.clone();
        let calls = Arc::new(StdMutex::new(Vec::new()));
        let recorded = calls.clone();
        let worker = tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                let method = request["method"].as_str().unwrap();
                recorded.lock().unwrap().push(method.into());
                let result = match method {
                    "systemCapabilities" => Some(Ok(json!(["SMS"]))),
                    "systemSmsIdsFacts" => Some(Ok(json!(ids))),
                    "systemSmsRowsFacts" => Some(Ok(
                        json!({"items":ids.iter().map(|id| json!({"id":id,"body":"archived","address":"sender","addressCandidates":[{"value":"sender","isSender":true}],"parts":[{"path":"text","contentType":"text/plain","name":"","fileName":"","hasData":false,"text":"archived"}],"date":"2026-01-01T00:00:00Z","threadId":"9","isMms":id.starts_with("mms_")})).collect::<Vec<_>>(),"canonicalAddresses":[],"ownNumbers":[]}),
                    )),
                    "systemDeleteSmsRows" => {
                        assert_eq!(request["params"]["ids"], json!(ids));
                        delete()
                    }
                    other => panic!("unexpected platform operation {other}"),
                };
                if let Some(result) = result {
                    let reply = match result {
                        Ok(result) => json!({"id":request["id"],"result":result}),
                        Err(error) => json!({"id":request["id"],"error":error}),
                    };
                    replies.reply(generation, reply).unwrap();
                }
            }
        });
        Self {
            directory,
            service,
            host,
            generation,
            worker,
            calls,
        }
    }

    fn preserve_existing_trash(&self) {
        self.service
            .db
            .trashed_message_save_many(&[TrashedMessageRow {
                message_id: "8".into(),
                is_mms: true,
                trashed_at: "2026-01-01T00:00:00.000Z".into(),
            }])
            .unwrap();
    }

    fn trashed(&self) -> Vec<String> {
        let mut ids = self.service.db.trashed_message_ids().unwrap();
        ids.sort();
        ids
    }

    async fn finish(self) {
        self.host.disconnect(self.generation);
        self.worker.await.unwrap();
    }
}

#[tokio::test]
async fn partial_deletion_archives_once_batches_the_sdk_and_preserves_existing_trash() {
    let fixture = Fixture::new(vec!["1".into(), "2".into(), "mms_8".into()], || {
        Some(Ok(json!({"deletedIds":["1"],"failedIds":["2","mms_8"]})))
    });
    fixture.preserve_existing_trash();
    assert_eq!(
        fixture
            .service
            .execute(Action::Delete, "all:true".into())
            .await
            .unwrap(),
        1
    );
    assert_eq!(fixture.trashed(), vec!["1", "mms_8"]);
    let stamp: String = fixture
        .service
        .db
        .with_conn(|connection| {
            connection.query_row(
                "SELECT trashed_at FROM trashed_sms WHERE message_id='8' AND is_mms=1",
                [],
                |row| row.get(0),
            )
        })
        .unwrap();
    assert_eq!(stamp, "2026-01-01T00:00:00.000Z");
    assert_eq!(
        *fixture.calls.lock().unwrap(),
        [
            "systemCapabilities",
            "systemSmsIdsFacts",
            "systemCapabilities",
            "systemSmsRowsFacts",
            "systemDeleteSmsRows"
        ]
    );
    assert_eq!(
        std::fs::read_dir(fixture.directory.path().join("sms-trash"))
            .unwrap()
            .count(),
        2
    );
    fixture.finish().await;
}

#[tokio::test]
async fn invalid_receipts_and_transport_failures_restore_only_new_shadow_rows() {
    for receipt in [
        Ok(json!({"deletedIds":["1","1"],"failedIds":["2","mms_8"]})),
        Ok(json!({"deletedIds":["1"],"failedIds":["mms_8"]})),
        Ok(json!({"deletedIds":["1","99"],"failedIds":["2","mms_8"]})),
        Err("SDK disconnected".into()),
    ] {
        let fixture = Fixture::new(vec!["1".into(), "2".into(), "mms_8".into()], move || {
            Some(receipt.clone())
        });
        fixture.preserve_existing_trash();
        assert!(
            fixture
                .service
                .execute(Action::Delete, "all:true".into())
                .await
                .is_err()
        );
        assert_eq!(fixture.trashed(), vec!["mms_8"]);
        fixture.finish().await;
    }
}

#[tokio::test]
async fn cancellation_rolls_back_new_shadow_rows_before_releasing_the_mutation_lock() {
    let reached = Arc::new(tokio::sync::Notify::new());
    let sent = reached.clone();
    let fixture = Fixture::new(vec!["1".into(), "mms_8".into()], move || {
        sent.notify_one();
        None
    });
    fixture.preserve_existing_trash();
    let service = fixture.service.clone();
    let task =
        tokio::spawn(async move { service.execute(Action::Delete, "all:true".into()).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), reached.notified())
        .await
        .expect("provider deletion was not reached");
    assert_eq!(fixture.trashed(), vec!["1", "mms_8"]);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(fixture.trashed(), vec!["mms_8"]);
    fixture.finish().await;
}

#[tokio::test]
async fn an_archive_failure_never_marks_or_deletes_provider_rows() {
    let fixture = Fixture::new(vec!["1".into()], || {
        panic!("provider deletion must not run before the archive exists")
    });
    std::fs::write(fixture.directory.path().join("sms-trash"), b"blocked").unwrap();
    assert!(
        fixture
            .service
            .execute(Action::Delete, "all:true".into())
            .await
            .is_err()
    );
    assert!(fixture.trashed().is_empty());
    assert_eq!(fixture.calls.lock().unwrap().len(), 4);
    fixture.finish().await;
}

#[tokio::test]
async fn trash_and_restore_change_rust_state_without_a_host_mutation_or_http_callback() {
    let fixture = Fixture::new(vec!["1".into(), "2".into()], || {
        panic!("trash and restore must not delete SDK rows")
    });
    assert_eq!(
        fixture
            .service
            .execute(Action::Trash, "ids:1,2".into())
            .await
            .unwrap(),
        2
    );
    assert_eq!(fixture.trashed(), vec!["1", "2"]);
    assert_eq!(
        fixture
            .service
            .execute(Action::Restore, "ids:1,2".into())
            .await
            .unwrap(),
        2
    );
    assert!(fixture.trashed().is_empty());
    assert_eq!(
        *fixture.calls.lock().unwrap(),
        [
            "systemCapabilities",
            "systemSmsIdsFacts",
            "systemCapabilities",
            "systemSmsIdsFacts"
        ]
    );
    fixture.finish().await;
}

#[tokio::test]
async fn missing_capability_does_not_read_or_mutate_provider_rows() {
    let directory = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&directory.path().join("data.db")).unwrap());
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let host = Arc::new(Host::default());
    let service = Mutations::new(db.clone(), host.clone(), prefs);
    let (generation, mut requests) = host.connect();
    let replies = host.clone();
    let worker = tokio::spawn(async move {
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "systemCapabilities");
        replies
            .reply(generation, json!({"id":request["id"],"result":[]}))
            .unwrap();
        assert!(requests.recv().await.is_none());
    });
    assert!(
        service
            .execute(Action::Delete, "all:true".into())
            .await
            .unwrap_err()
            .to_string()
            .contains("capability")
    );
    assert!(db.trashed_message_ids().unwrap().is_empty());
    host.disconnect(generation);
    worker.await.unwrap();
}

#[test]
fn shadow_trash_counts_duplicates_once_and_preserves_existing_timestamps() {
    let directory = tempfile::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    assert_eq!(
        save_trash(&db, &["1".into(), "mms_2".into(), "1".into()]).unwrap(),
        2
    );
    let stamp = || {
        db.with_conn(|connection| {
            connection.query_row(
                "SELECT trashed_at FROM trashed_sms WHERE message_id='1' AND is_mms=0",
                [],
                |row| row.get::<_, String>(0),
            )
        })
        .unwrap()
    };
    let before = stamp();
    assert_eq!(save_trash(&db, &["1".into()]).unwrap(), 0);
    assert_eq!(stamp(), before);
    assert_eq!(
        db.trashed_message_delete_by_ids(&["1".into(), "missing".into(), "1".into()])
            .unwrap(),
        1
    );
    assert_eq!(db.trashed_message_ids().unwrap(), vec!["mms_2"]);
}
