use super::*;
use plain_rs::features::audit_events::*;

#[tokio::test]
async fn public_events_read_the_core_database_without_host_or_legacy_store() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state
        .audit_events
        .record(
            AuditEvent::UpdateDeviceName(DeviceNameEventData {
                old_name: "Old".into(),
                new_name: "ÄBC".into(),
            }),
            Some(plain_rs_client()),
        )
        .unwrap();
    state.audit_events.flush().await.unwrap();
    let id = state.db.audit_events(0, 10, Some("äbc")).unwrap()[0]
        .id
        .clone();
    let result = core.native_graphql().unwrap().execute(plain_graphql::Request::new(
        r#"{ events(query:"text:äbc",offset:0,limit:10) { id type data { __typename ... on DeviceNameEventData { oldName newName } } clientId clientName clientIp osName osVersion appName appVersion createdAt } }"#
    )).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    assert_eq!(data["events"][0]["id"], id);
    assert_eq!(data["events"][0]["clientId"], "706c6169-26be-890e-b404-adf1389070db");
    assert_eq!(data["events"][0]["clientName"], "Fixture client");
    assert_eq!(data["events"][0]["clientIp"], "192.0.2.24");
    assert_eq!(data["events"][0]["osName"], "Windows");
    assert_eq!(data["events"][0]["osVersion"], "11");
    assert_eq!(data["events"][0]["appName"], "Fixture browser");
    assert_eq!(data["events"][0]["appVersion"], "142");
    state
        .audit_events
        .record(
            AuditEvent::Mount(VolumeEventData {
                volume: EventVolume {
                    id: None,
                    name: None,
                    device_path: None,
                    mount_path: "/mnt/example".into(),
                    file_system: None,
                },
            }),
            None,
        )
        .unwrap();
    state.audit_events.flush().await.unwrap();
    let system_id = state.db.audit_events(0, 10, Some("/mnt/example")).unwrap()[0]
        .id
        .clone();
    let result = core
        .native_graphql()
        .unwrap()
        .execute(plain_graphql::Request::new(
            r#"{ events(query:"text:/mnt/example",offset:0,limit:10) { id clientId } }"#,
        ))
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    assert_eq!(data["events"][0]["id"], system_id);
    assert!(data["events"][0]["clientId"].is_null());
    state
        .db
        .with_conn(|conn| conn.execute_batch("DROP TABLE events"))
        .unwrap();
    let result = core
        .native_graphql()
        .unwrap()
        .execute(plain_graphql::Request::new(
            r#"{ events(query:"",offset:0,limit:10) { id } }"#,
        ))
        .await;
    assert!(!result.errors.is_empty());
    core.stop_core().await;
}

#[tokio::test]
async fn public_disk_commands_reach_the_registered_provider() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let provider = tokio::spawn(async move {
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "systemCapabilities");
        host.reply(
            generation,
            plain_libs::serde_json::json!({"id":request["id"],"result":["DISK_MANAGER"]}),
        )
        .unwrap();
        let request = requests.recv().await.unwrap();
        assert_eq!(
            request["method"],
            plain_rs::infrastructure::host::commands::SYSTEM_DISK_FORMAT
        );
        assert_eq!(request["params"]["path"], "/dev/example");
        host.reply(
            generation,
            plain_libs::serde_json::json!({"id":request["id"],"error":"format refused"}),
        )
        .unwrap();
    });
    let executor = core.native_graphql().unwrap();
    let result = executor
        .execute("mutation { formatDisk(path:\"/dev/example\") }".into())
        .await;
    assert_eq!(result.errors[0].message, "format refused");
    state.audit_events.flush().await.unwrap();
    let events = state.db.audit_events(0, 10, None).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].r#type, EventType::FormatDiskFailed);
    assert_eq!(
        events[0].client_id.as_deref(),
        Some(state.native_client_id().unwrap().as_str())
    );
    assert!(
        matches!(&events[0].data, EventData::DiskFormatFailed(data) if data.disk.path == "/dev/example" && data.file_system == "ext4" && data.failure.reason == ErrorCode::FormatDiskFailed)
    );
    provider.await.unwrap();
    core.stop_core().await;
}

#[tokio::test]
async fn public_mount_alias_is_database_owned_and_ignores_retired_preferences() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    std::fs::write(
        directory.path().join("prefs.json"),
        r#"{"volume_alias":{"fixture-mount":"Retired name"}}"#,
    )
    .unwrap();
    let source = directory.path().join("generated-volume");
    std::fs::create_dir(&source).unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state.file_platform.use_sdk_for_test();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let facts = plain_libs::serde_json::json!([{"id":"fixture-mount","name":"Fixture volume","path":source,"mountPoint":source,"totalBytes":100,"freeBytes":60,"driveType":"USB_STORAGE"}]);
    let provider = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemMountFacts" => facts.clone(),
                "systemCapabilities" => plain_libs::serde_json::json!([]),
                "systemAccessFacts" => plain_libs::serde_json::json!({"granted":{"FILES":true}}),
                method => panic!("unexpected {method}"),
            };
            host.reply(
                generation,
                plain_libs::serde_json::json!({"id":request["id"],"result":result}),
            )
            .unwrap();
        }
    });
    let prefs_path = directory.path().join("prefs.json");
    let before = std::fs::read(&prefs_path).unwrap();
    let executor = core.native_graphql().unwrap();
    let query = executor.execute("{ mounts { id alias } }".into()).await;
    assert!(query.errors.is_empty(), "{:?}", query.errors);
    assert_eq!(query.data.into_json().unwrap()["mounts"][0]["alias"], "");
    for (input, expected) in [(" Photos ", "Photos"), ("", "")] {
        let response = executor
            .execute(
                plain_graphql::Request::new(
                    r#"mutation($alias:String!){setMountAlias(id:"fixture-mount",alias:$alias)}"#,
                )
                .variables(plain_graphql::Variables::from_json(
                    plain_libs::serde_json::json!({"alias":input}),
                )),
            )
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert!(
            std::fs::read(&prefs_path).unwrap() == before,
            "renaming a volume must not write preferences"
        );
        let volume = state
            .db
            .volumes()
            .unwrap()
            .into_iter()
            .find(|volume| std::path::Path::new(&source).starts_with(&volume.path))
            .unwrap();
        let alias: String = state
            .db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT alias FROM volumes WHERE id=?1",
                    [&volume.id],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_eq!(alias, expected);
        let query = executor.execute("{ mounts { id alias } }".into()).await;
        assert!(query.errors.is_empty(), "{:?}", query.errors);
        assert_eq!(
            query.data.into_json().unwrap()["mounts"][0]["alias"],
            expected
        );
    }
    let rejected = executor
        .execute(r#"mutation { setMountAlias(id:"missing-volume",alias:"Rejected") }"#.into())
        .await;
    assert!(!rejected.errors.is_empty());
    assert!(std::fs::read(&prefs_path).unwrap() == before);
    core.stop_core().await;
    provider.abort();
}

#[tokio::test]
async fn unsupported_platform_operations_only_read_capabilities_and_leave_state_unchanged() {
    for (query, capability) in [
        ("{ disks { id } }", "DISK_MANAGER"),
        (
            r#"mutation { formatDisk(path:"/dev/example") }"#,
            "DISK_MANAGER",
        ),
        ("{ appUpdate { currentVersion } }", "APP_UPDATES"),
    ] {
        let directory = plain_libs::temporary::tempdir().unwrap();
        let core = crate::server::ContentServer::open(directory.path()).unwrap();
        let state = core.runtime_state();
        let before = state.prefs.entries();
        let (generation, mut requests) = state.host.connect();
        let host = state.host.clone();
        let provider = tokio::spawn(async move {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "systemCapabilities");
            host.reply(
                generation,
                plain_libs::serde_json::json!({"id":request["id"],"result":[]}),
            )
            .unwrap();
            requests
        });
        let response = core.native_graphql().unwrap().execute(query.into()).await;
        assert_eq!(response.errors.len(), 1, "{query}: {:?}", response.errors);
        assert_eq!(
            response.errors[0].message,
            format!("unsupported capability {capability}")
        );
        let mut requests = provider.await.unwrap();
        assert!(requests.try_recv().is_err());
        assert_eq!(state.prefs.entries(), before);
        assert!(state.db.audit_events(0, 10, None).unwrap().is_empty());
        core.stop_core().await;
    }
}

#[tokio::test]
async fn disk_queries_read_native_rust_after_one_capability_check() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = calls.clone();
    let provider = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let method = request["method"].as_str().unwrap().to_owned();
            observed.lock().unwrap().push(method.clone());
            let result = if method == "systemCapabilities" {
                plain_libs::serde_json::json!(["DISK_MANAGER"])
            } else {
                plain_libs::serde_json::json!({"disks":[]})
            };
            host.reply(
                generation,
                plain_libs::serde_json::json!({"id":request["id"],"result":result}),
            )
            .unwrap();
        }
    });
    let response = core
        .native_graphql()
        .unwrap()
        .execute("{ disks { id name path sizeBytes removable model } }".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    for disk in data["disks"].as_array().unwrap() {
        assert!(!disk["id"].as_str().unwrap().is_empty());
        assert!(!disk["path"].as_str().unwrap().is_empty());
        assert!(disk["sizeBytes"].as_i64().unwrap() >= 0);
    }
    assert_eq!(*calls.lock().unwrap(), vec!["systemCapabilities"]);
    assert!(state.db.audit_events(0, 10, None).unwrap().is_empty());
    provider.abort();
    let _ = provider.await;
    core.stop_core().await;
}

fn plain_rs_client() -> plain_rs::infrastructure::db::AuditEventClient {
    plain_rs::infrastructure::db::AuditEventClient {
        client_id: "706c6169-26be-890e-b404-adf1389070db".into(),
        client_name: "Fixture client".into(),
        client_ip: "192.0.2.24".into(),
        os_name: "Windows".into(),
        os_version: "11".into(),
        app_name: "Fixture browser".into(),
        app_version: "142".into(),
    }
}

#[tokio::test]
async fn successful_format_records_one_success_event_without_waiting_for_the_writer() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let provider = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let value = match request["method"].as_str().unwrap() {
                "systemCapabilities" => plain_libs::serde_json::json!(["DISK_MANAGER"]),
                "systemDiskFormat" => {
                    assert_eq!(request["params"]["path"], "/dev/generated-disk");
                    plain_libs::serde_json::json!(true)
                }
                method => panic!("unexpected {method}"),
            };
            host.reply(
                generation,
                plain_libs::serde_json::json!({"id":request["id"],"result":value}),
            )
            .unwrap();
            if request["method"] == "systemDiskFormat" {
                break;
            }
        }
    });
    let response = core
        .native_graphql()
        .unwrap()
        .execute("mutation { formatDisk(path:\"/dev/generated-disk\") }".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["formatDisk"], true);
    provider.await.unwrap();
    state.audit_events.flush().await.unwrap();
    let rows = state.db.audit_events(0, 10, None).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].r#type, EventType::FormatDisk);
    assert!(
        matches!(&rows[0].data, EventData::DiskFormat(data) if data.disk.path == "/dev/generated-disk" && data.file_system == "ext4")
    );
    core.stop_core().await;
}
