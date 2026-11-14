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
    let source = plain_libs::temporary::tempdir().unwrap();
    let source_path = source.path().canonicalize().unwrap();
    let id = state.db.register_volume(&source_path).unwrap().unwrap();
    let query = native.execute(Request::new("{ fileIndexVolumes { id path indexMode includedDirs excludedDirs isAvailable } fileIndexDefaults { excludedDirs excludedNames } }")).await;
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
    let volume = data["fileIndexVolumes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|volume| volume["id"] == id)
        .unwrap();
    assert_eq!(volume["indexMode"], "UNCONFIGURED");
    state
        .db
        .with_conn(|conn| conn.execute("UPDATE volumes SET index_mode='DISABLED'", []))
        .unwrap();
    let set = native.execute(Request::new(
        "mutation($id:ID!,$dirs:[String!]!){setFileIndexVolume(id:$id,indexMode:DISABLED,includedDirs:$dirs,excludedDirs:[])}"
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
    std::fs::write(source_path.join("included.txt"), "generated index fixture").unwrap();
    std::fs::create_dir(source_path.join("excluded")).unwrap();
    std::fs::write(
        source_path.join("excluded/ignored.txt"),
        "generated exclusion fixture",
    )
    .unwrap();
    let configured = native.execute(Request::new(
        "mutation($id:ID!,$dirs:[String!]!,$excluded:[String!]!){setFileIndexVolume(id:$id,indexMode:ALLOWLIST,includedDirs:$dirs,excludedDirs:$excluded)}"
    ).variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({
        "id":id,"dirs":[source_path.to_str().unwrap()],
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
