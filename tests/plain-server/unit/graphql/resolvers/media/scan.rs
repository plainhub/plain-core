use plain_graphql::Request;

#[cfg(feature = "media")]
#[tokio::test]
async fn native_media_controls_share_the_main_contract_and_remote_permissions() {
    let directory = tempfile::tempdir().unwrap();
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
                "systemCapabilities" => serde_json::json!(["MEDIA_SCAN"]),
                "systemAccessFacts" => {
                    serde_json::json!({"granted":{"FILES":true}})
                }
                method => panic!("unexpected {method}"),
            };
            host.reply(
                generation,
                serde_json::json!({"id":request["id"],"result":result}),
            )
            .unwrap();
        }
    });
    let native = core.native_graphql().unwrap();
    let source = tempfile::tempdir().unwrap();
    let source_path = source.path().canonicalize().unwrap();
    let id = state.db.register_volume(&source_path).unwrap();
    let query = native.execute(Request::new("{ fileIndexVolumes { id path indexMode includedDirs excludedDirs isAvailable } fileIndexDefaults { excludedDirs excludedNames } }")).await;
    assert!(query.errors.is_empty(), "{:?}", query.errors);
    let data = query.data.into_json().unwrap();
    assert!(
        data["fileIndexDefaults"]["excludedDirs"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(
                plain_rs::features::files::index_scope::normalized(&state.directory)
            ))
    );
    assert!(
        data["fileIndexDefaults"]["excludedNames"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("node_modules"))
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
    ).variables(plain_graphql::Variables::from_json(serde_json::json!({"id":id,"dirs":[source_path.to_str().unwrap()]})))).await;
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
        plain_rs::infrastructure::db::IndexMode::Disabled
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
        serde_json::json!([plain_rs::features::files::index_scope::normalized(
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
        .execute(Request::new("{ scanProgress { state } }"))
        .await;
    assert!(main.errors.is_empty(), "{:?}", main.errors);
    provider.abort();
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn public_scans_require_opt_in_and_grants_then_read_the_same_core() {
    let directory = tempfile::tempdir().unwrap();
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
        Request::new("{ fileIndexVolumes { id indexMode } scanProgress { indexed total state } }")
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
                    serde_json::json!({"id":request["id"],"result":["MEDIA_SCAN"]}),
                )
                .unwrap();
                continue;
            }
            assert_eq!(request["method"], "systemAccessFacts");
            assert_eq!(request["params"]["features"], serde_json::json!(["FILES"]));
            host.reply(
                generation,
                serde_json::json!({"id":request["id"], "result":{"granted":{"FILES":granted}}}),
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
    assert_eq!(data["scanProgress"]["state"], "IDLE");
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
