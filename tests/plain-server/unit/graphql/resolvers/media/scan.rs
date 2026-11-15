use plain_graphql::Request;

#[cfg(feature = "media")]
#[tokio::test]
async fn native_media_controls_share_the_main_contract_and_remote_permissions() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state
        .media_scans
        .set_defaults(plain_rs::features::files::index_scope::Defaults {
            excluded_directories: Vec::new(),
            excluded_names: plain_rs::features::files::index_scope::defaults().excluded_names,
        })
        .unwrap();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let provider = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => plain_libs::serde_json::json!(["MEDIA_SCAN"]),
                "systemAccessFacts" => {
                    plain_libs::serde_json::json!({"granted":{"FILES":true}})
                }
                method => panic!("unexpected {method}"),
            };
            host.reply(
                generation,
                plain_libs::serde_json::json!({"id":request["id"],"result":result}),
            )
            .unwrap();
        }
    });
    let native = core.native_graphql().unwrap();
    let source = plain_libs::temporary::Builder::new()
        .prefix("plain-index-scan-")
        .tempdir_in(
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap()),
        )
        .unwrap();
    let source_path = source.path().canonicalize().unwrap();
    let id = state.db.register_volume(&source_path).unwrap().unwrap();
    let query = native.execute(Request::new("{ fileIndexVolumes { id path indexMode includedDirs excludedDirs isAvailable lastScanAt lastScanStatus lastScanCount lastScanTotal } fileIndexDefaults { excludedDirs excludedNames excludedProfileDirs { profilesDir excludedDir exceptDirs } mediaExcludedDirSuffixes } }")).await;
    assert!(query.errors.is_empty(), "{:?}", query.errors);
    let data = query.data.into_json().unwrap();
    assert!(
        data["fileIndexDefaults"]["excludedDirs"]
            .as_array()
            .unwrap()
            .contains(&plain_libs::serde_json::json!(
                plain_rs::features::files::index_scope::normalized(&state.directory)
            ))
    );
    assert!(
        data["fileIndexDefaults"]["excludedNames"]
            .as_array()
            .unwrap()
            .contains(&plain_libs::serde_json::json!("node_modules"))
    );
    let rules = &data["fileIndexDefaults"];
    #[cfg(target_os = "macos")]
    {
        assert_eq!(
            rules["excludedProfileDirs"],
            plain_libs::serde_json::json!([{
                "profilesDir": "/Users/", "excludedDir": "Library",
                "exceptDirs": ["Library/Mobile Documents/com~apple~CloudDocs"]
            }])
        );
        assert_eq!(
            rules["mediaExcludedDirSuffixes"],
            plain_libs::serde_json::json!([".photoslibrary", ".app", ".bundle", ".framework"])
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        assert_eq!(
            rules["excludedProfileDirs"],
            plain_libs::serde_json::json!([])
        );
        assert_eq!(
            rules["mediaExcludedDirSuffixes"],
            plain_libs::serde_json::json!([])
        );
    }
    let volume = data["fileIndexVolumes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|volume| volume["id"] == id)
        .unwrap();
    assert_eq!(volume["indexMode"], "UNCONFIGURED");
    for field in [
        "lastScanAt",
        "lastScanStatus",
        "lastScanCount",
        "lastScanTotal",
    ] {
        assert!(volume[field].is_null(), "{field}");
    }
    for (status, wire, count, total) in [
        (
            plain_rs::infrastructure::db::FileIndexScanStatus::Completed,
            "COMPLETED",
            0,
            Some(0),
        ),
        (
            plain_rs::infrastructure::db::FileIndexScanStatus::Canceled,
            "CANCELED",
            2,
            None,
        ),
        (
            plain_rs::infrastructure::db::FileIndexScanStatus::Failed,
            "FAILED",
            3,
            Some(5),
        ),
    ] {
        state
            .db
            .record_volume_scan(&id, status, count, total)
            .unwrap();
        let response = native
            .execute(Request::new(
                "{ fileIndexVolumes { id lastScanAt lastScanStatus lastScanCount lastScanTotal } }",
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        let volume = data["fileIndexVolumes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|volume| volume["id"] == id)
            .unwrap();
        assert_eq!(volume["lastScanStatus"], wire);
        assert_eq!(volume["lastScanCount"], count);
        assert_eq!(
            volume["lastScanTotal"],
            plain_libs::serde_json::json!(total)
        );
        plain_libs::chrono::DateTime::parse_from_rfc3339(volume["lastScanAt"].as_str().unwrap())
            .unwrap();
    }

    state
        .db
        .with_conn(|conn| conn.execute("UPDATE volumes SET index_mode='DISABLED'", []))
        .unwrap();
    let set = native.execute(Request::new(
        "mutation($id:ID!,$dirs:[String!]!){setFileIndexVolume(input:{volumeId:$id,indexMode:DISABLED,includedDirs:$dirs,excludedDirs:[]})}"
    ).variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({"id":id,"dirs":[source_path.to_str().unwrap()]})))).await;
    assert!(set.errors.is_empty(), "{:?}", set.errors);
    state.media_scans.wait_background().await.unwrap();
    let saved = state
        .db
        .volumes()
        .unwrap()
        .into_iter()
        .find(|volume| volume.id == id)
        .unwrap();
    assert_eq!(
        saved.index_mode,
        plain_rs::infrastructure::db::FileIndexMode::Disabled
    );
    assert_eq!(saved.included_dirs.len(), 1);
    let query = native
        .execute(Request::new("{ fileIndexVolumes { id includedDirs } }"))
        .await;
    assert!(query.errors.is_empty(), "{:?}", query.errors);
    let data = query.data.into_json().unwrap();
    let volume = data["fileIndexVolumes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|volume| volume["id"] == id)
        .unwrap();
    assert_eq!(
        volume["includedDirs"],
        plain_libs::serde_json::json!([plain_rs::features::files::index_scope::normalized(
            &source_path
        )])
    );
    let state = core.runtime_state();
    let remote = crate::graphql::executor::execute_graphql(
        &state.public,
        Request::new("{ fileIndexVolumes { id } }"),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!remote.errors.is_empty());
    let main = native
        .execute(Request::new("{ fileIndexScanProgress { state } }"))
        .await;
    assert!(main.errors.is_empty(), "{:?}", main.errors);
    let first = state.media_scans.enqueue_volume(&id).unwrap().unwrap();
    let second = state
        .media_scans
        .enqueue_volume("generated-queued-volume")
        .unwrap()
        .unwrap();
    let queued = native.execute(Request::new("{ fileIndexScanProgress(volumeId: \"generated-queued-volume\") { volumeId state indexed total } }")).await;
    assert!(queued.errors.is_empty(), "{:?}", queued.errors);
    assert_eq!(
        queued.data.into_json().unwrap()["fileIndexScanProgress"]["state"],
        "QUEUED"
    );
    for (operation, volume_id, expected_first, expected_second) in [
        ("pauseFileIndex", id.as_str(), "PAUSED", "RUNNING"),
        ("resumeFileIndex", id.as_str(), "QUEUED", "RUNNING"),
        (
            "stopFileIndex",
            "generated-queued-volume",
            "QUEUED",
            "STOPPED",
        ),
    ] {
        let response = native
            .execute(
                Request::new(format!(
                    "mutation($volumeId:ID){{{operation}(volumeId:$volumeId)}}"
                ))
                .variables(plain_graphql::Variables::from_json(
                    plain_libs::serde_json::json!({"volumeId":volume_id}),
                )),
            )
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let progress = native.execute(Request::new("query($id:ID){first:fileIndexScanProgress(volumeId:$id){volumeId state} second:fileIndexScanProgress(volumeId:\"generated-queued-volume\"){volumeId state}}")
            .variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({"id":id})))).await;
        assert!(progress.errors.is_empty(), "{:?}", progress.errors);
        let progress = progress.data.into_json().unwrap();
        assert_eq!(progress["first"]["volumeId"], id);
        assert_eq!(progress["first"]["state"], expected_first);
        assert_eq!(progress["second"]["state"], expected_second);
    }
    second.finish(Ok(())).unwrap();
    let stopped = native
        .execute(Request::new("mutation { stopFileIndex }"))
        .await;
    assert!(stopped.errors.is_empty(), "{:?}", stopped.errors);
    first.finish(Ok(())).unwrap();
    for volume_id in [id.as_str(), "missing-volume"] {
        let denied = native
            .execute(
                Request::new("mutation($volumeId:ID){rebuildFileIndex(volumeId:$volumeId)}")
                    .variables(plain_graphql::Variables::from_json(
                        plain_libs::serde_json::json!({"volumeId":volume_id}),
                    )),
            )
            .await;
        assert!(
            !denied.errors.is_empty(),
            "disabled or unknown volume accepted"
        );
    }
    std::fs::create_dir(source_path.join("included")).unwrap();
    std::fs::write(
        source_path.join("included/included.txt"),
        "generated index fixture",
    )
    .unwrap();
    std::fs::create_dir(source_path.join("excluded")).unwrap();
    std::fs::write(
        source_path.join("excluded/ignored.txt"),
        "generated exclusion fixture",
    )
    .unwrap();
    let configured = native.execute(Request::new(
        "mutation($id:ID!,$dirs:[String!]!,$excluded:[String!]!){setFileIndexVolume(input:{volumeId:$id,indexMode:ALLOWLIST,includedDirs:$dirs,excludedDirs:$excluded})}"
    ).variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({
        "id":id,"dirs":[source_path.join("included").to_str().unwrap()],
        "excluded":[source_path.join("excluded").to_str().unwrap()]
    })))).await;
    assert!(configured.errors.is_empty(), "{:?}", configured.errors);
    state.media_scans.wait_background().await.unwrap();
    for (document, variables) in [
        (
            "mutation { rebuildFileIndex }",
            plain_libs::serde_json::json!({}),
        ),
        (
            "mutation($volumeId:ID){rebuildFileIndex(volumeId:$volumeId)}",
            plain_libs::serde_json::json!({"volumeId":null}),
        ),
        (
            "mutation($volumeId:ID){rebuildFileIndex(volumeId:$volumeId)}",
            plain_libs::serde_json::json!({"volumeId":id}),
        ),
    ] {
        let rebuilt = native
            .execute(
                Request::new(document).variables(plain_graphql::Variables::from_json(variables)),
            )
            .await;
        assert!(rebuilt.errors.is_empty(), "{:?}", rebuilt.errors);
        assert_eq!(rebuilt.data.into_json().unwrap()["rebuildFileIndex"], true);
        state.media_scans.wait_background().await.unwrap();
        let files = native.execute(Request::new(
            "{ indexedFiles(query:\"\",offset:0,limit:100,sortBy:NAME_ASC){name} fileIndexScanProgress{state} }"
        )).await;
        assert!(files.errors.is_empty(), "{:?}", files.errors);
        let files = files.data.into_json().unwrap();
        let names = files["indexedFiles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(names.contains(&"included.txt"));
        assert!(!names.contains(&"ignored.txt"));
        assert_eq!(files["fileIndexScanProgress"]["state"], "IDLE");
    }
    provider.abort();
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn public_scans_require_opt_in_and_grants_then_read_the_same_core() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state
        .media_scans
        .set_defaults(plain_rs::features::files::index_scope::Defaults {
            excluded_directories: Vec::new(),
            excluded_names: Vec::new(),
        })
        .unwrap();
    let request = || {
        Request::new(
            "{ fileIndexVolumes { id indexMode } fileIndexScanProgress { indexed total state } }",
        )
    };
    let denied = crate::graphql::executor::execute_graphql(
        &state.public,
        request(),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!denied.errors.is_empty());
    state
        .prefs
        .set(
            plain_rs::features::prefs::pref_keys::API_PERMISSIONS,
            vec![super::super::public::STORAGE],
        )
        .unwrap();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let grants = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let provider_grants = grants.clone();
    let (done, mut finish) = tokio::sync::oneshot::channel::<()>();
    let provider = tokio::spawn(async move {
        loop {
            let request = tokio::select! {
                _ = &mut finish => break,
                request = requests.recv() => request.unwrap(),
            };
            let granted = provider_grants.load(std::sync::atomic::Ordering::SeqCst);
            if request["method"] == "systemCapabilities" {
                host.reply(
                    generation,
                    plain_libs::serde_json::json!({"id":request["id"],"result":["MEDIA_SCAN"]}),
                )
                .unwrap();
                continue;
            }
            assert_eq!(request["method"], "systemAccessFacts");
            assert_eq!(
                request["params"]["features"],
                plain_libs::serde_json::json!(["FILES"])
            );
            host.reply(
                generation,
                plain_libs::serde_json::json!({"id":request["id"], "result":{"granted":{"FILES":granted}}}),
            )
            .unwrap();
        }
    });
    let allowed = crate::graphql::executor::execute_graphql(
        &state.public,
        request(),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(allowed.errors.is_empty(), "{:?}", allowed.errors);
    let data = allowed.data.into_json().unwrap();
    assert!(data["fileIndexVolumes"].is_array());
    assert_eq!(data["fileIndexScanProgress"]["state"], "IDLE");
    grants.store(false, std::sync::atomic::Ordering::SeqCst);
    let denied = crate::graphql::executor::execute_graphql(
        &state.public,
        request(),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!denied.errors.is_empty());
    done.send(()).unwrap();
    provider.await.unwrap();
    core.stop_core().await;
}

#[test]
fn core_volume_progress_events_preserve_the_volume_id_for_subscription_consumers() {
    let (events, mut received) = tokio::sync::broadcast::channel(32);
    let scans = std::sync::Arc::new(
        plain_rs::features::services::files::media_scans::MediaScans::new(
            events,
            std::env::temp_dir(),
        ),
    );
    let _session = scans.enqueue_volume("generated-volume").unwrap().unwrap();
    let event = received.try_recv().unwrap();
    let progress: super::FileIndexScanProgress =
        plain_libs::serde_json::from_str(&event.payload).unwrap();
    assert_eq!(
        progress.volume_id.as_ref().map(|id| id.as_str()),
        Some("generated-volume")
    );
    assert_eq!(
        progress.state,
        plain_rs::features::services::files::scanning::FileIndexScanState::Running
    );
    assert_eq!(progress.total, None);
}

#[test]
fn file_index_default_types_match_the_committed_main_schema() {
    use crate::graphql::schemas::public::{Mutation, PublicSchema, Query};
    let sdl = PublicSchema::build(
        Query::default(),
        Mutation::default(),
        crate::graphql::subscriptions::Subscription::default(),
    )
    .finish()
    .sdl();
    let committed = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../schema/api/media.graphqls"
    ));
    for name in ["FileIndexDefaults", "FileIndexProfileDir"] {
        let declaration = format!("type {name} {{");
        let actual = sdl
            .split_once(&declaration)
            .unwrap()
            .1
            .split_once("\n}")
            .unwrap()
            .0;
        let expected = committed
            .split_once(&declaration)
            .unwrap()
            .1
            .split_once("\n}")
            .unwrap()
            .0;
        assert_eq!(actual, expected, "{name}");
    }
}
