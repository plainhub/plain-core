use super::*;

#[tokio::test]
async fn public_samba_settings_and_mutation_use_core_preferences_and_explicit_host_methods() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let provider = tokio::spawn(async move {
        for method in [
            "systemCapabilities",
            SYSTEM_SAMBA_SERVICE_FACTS,
            "systemCapabilities",
            SYSTEM_SAMBA_SERVICE_FACTS,
            SYSTEM_SAMBA_APPLY,
        ] {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], method);
            let result = if method == "systemCapabilities" {
                serde_json::json!(["LAN_SHARE"])
            } else if method == SYSTEM_SAMBA_APPLY {
                assert_eq!(request["params"]["settings"]["enabled"], true);
                assert_eq!(request["params"]["settings"]["shares"][0]["auth"], "Guest");
                serde_json::Value::Bool(true)
            } else {
                serde_json::json!({"name":"smbd","active":true,"enabled":true})
            };
            host.reply(
                generation,
                serde_json::json!({"id":request["id"],"result":result}),
            )
            .unwrap();
        }
    });
    let query = core
        .native_graphql()
        .unwrap()
        .execute(async_graphql::Request::new(
            "{ sambaSettings { enabled serviceName serviceActive } }",
        ))
        .await;
    assert!(query.errors.is_empty(), "{:?}", query.errors);
    assert_eq!(
        query.data.into_json().unwrap()["sambaSettings"]["serviceActive"],
        true
    );
    let result = core.native_graphql().unwrap().execute(async_graphql::Request::new(
        r#"mutation { setSambaSettings(input:{enabled:true,shares:[{name:"media",sharePath:"/mnt/media",auth:GUEST,readOnly:true}]}) }"#
    )).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(
        plain_rs::prefs::samba::get_samba_settings(&state.prefs)
            .unwrap()
            .enabled
    );
    provider.await.unwrap();
    core.stop_core().await;
}

#[tokio::test]
async fn public_audit_events_read_the_core_database_without_host_or_legacy_store() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let id = state
        .db
        .audit_event_add("login", "ÄBC", Some("client"))
        .unwrap();
    let result = core.native_graphql().unwrap().execute(async_graphql::Request::new(
        r#"{ auditEvents(query:"text:äbc",offset:0,limit:10) { id type message clientId createdAt } }"#
    )).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    assert_eq!(data["auditEvents"][0]["id"], id);
    assert_eq!(data["auditEvents"][0]["clientId"], "client");
    let system_id = state
        .db
        .audit_event_add("mount", "/mnt/example", None)
        .unwrap();
    let result = core
        .native_graphql()
        .unwrap()
        .execute(async_graphql::Request::new(
            r#"{ auditEvents(query:"text:/mnt/example",offset:0,limit:10) { id clientId } }"#,
        ))
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    assert_eq!(data["auditEvents"][0]["id"], system_id);
    assert!(data["auditEvents"][0]["clientId"].is_null());
    state
        .db
        .with_conn(|conn| conn.execute_batch("DROP TABLE audit_events"))
        .unwrap();
    let result = core
        .native_graphql()
        .unwrap()
        .execute(async_graphql::Request::new(
            r#"{ auditEvents(query:"",offset:0,limit:10) { id } }"#,
        ))
        .await;
    assert!(!result.errors.is_empty());
    core.stop_core().await;
}

#[tokio::test]
async fn public_disk_and_hostname_commands_reach_the_registered_provider() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let provider = tokio::spawn(async move {
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "systemCapabilities");
        host.reply(
            generation,
            serde_json::json!({"id":request["id"],"result":["DISK_MANAGER"]}),
        )
        .unwrap();
        let request = requests.recv().await.unwrap();
        assert_eq!(
            request["method"],
            plain_rs::host::commands::SYSTEM_DISK_FORMAT
        );
        assert_eq!(request["params"]["path"], "/dev/example");
        host.reply(
            generation,
            serde_json::json!({"id":request["id"],"error":"format refused"}),
        )
        .unwrap();
        let request = requests.recv().await.unwrap();
        assert_eq!(
            request["method"],
            plain_rs::host::commands::SYSTEM_HOSTNAME_SET
        );
        assert_eq!(request["params"]["name"], "my-server");
        host.reply(
            generation,
            serde_json::json!({"id":request["id"],"result":true}),
        )
        .unwrap();
    });
    let executor = core.native_graphql().unwrap();
    let result = executor
        .execute("mutation { formatDisk(path:\"/dev/example\") }".into())
        .await;
    assert_eq!(result.errors[0].message, "format refused");
    let result = executor
        .execute("mutation { setHostname(name:\" My_Server \") }".into())
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.data.into_json().unwrap()["setHostname"], true);
    provider.await.unwrap();
    let rows = state.db.audit_events(0, 10, None).unwrap();
    assert!(
        rows.iter()
            .any(|row| row.r#type == "set_hostname" && row.message == "my-server")
    );
    core.stop_core().await;
}

#[tokio::test]
async fn public_mount_alias_uses_the_same_preferences_as_the_platform_adapter() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let result = core
        .native_graphql()
        .unwrap()
        .execute("mutation { setMountAlias(id:\"volume-example\",alias:\" Photos \") }".into())
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        plain_rs::prefs::storage::get_map(core.preferences().unwrap())["volume-example"],
        "Photos"
    );
    core.stop_core().await;
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
        ("{ sambaSettings { enabled } }", "LAN_SHARE"),
        (
            "mutation { setSambaSettings(input:{enabled:true,shares:[]}) }",
            "LAN_SHARE",
        ),
        (
            r#"mutation { setSambaUserPassword(password:"example") }"#,
            "LAN_SHARE",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let core = crate::server::ContentServer::open(directory.path()).unwrap();
        let state = core.runtime_state();
        let before =
            serde_json::to_value(plain_rs::prefs::samba::get_samba_settings(&state.prefs).unwrap())
                .unwrap();
        let (generation, mut requests) = state.host.connect();
        let host = state.host.clone();
        let provider = tokio::spawn(async move {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "systemCapabilities");
            host.reply(
                generation,
                serde_json::json!({"id":request["id"],"result":[]}),
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
        assert_eq!(
            serde_json::to_value(plain_rs::prefs::samba::get_samba_settings(&state.prefs).unwrap())
                .unwrap(),
            before
        );
        assert!(state.db.audit_events(0, 10, None).unwrap().is_empty());
        core.stop_core().await;
    }
}

#[test]
fn hostname_sanitization_is_shared() {
    assert_eq!(plain_rs::platform_hostname("  Foo_. Bar-- "), "foo-bar");
    assert_eq!(plain_rs::platform_hostname("!.."), "");
}

#[tokio::test]
async fn disk_queries_read_native_rust_after_one_capability_check() {
    let directory = tempfile::tempdir().unwrap();
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
                serde_json::json!(["DISK_MANAGER"])
            } else {
                serde_json::json!({"disks":[]})
            };
            host.reply(
                generation,
                serde_json::json!({"id":request["id"],"result":result}),
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
