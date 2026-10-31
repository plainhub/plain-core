use plain_rs::services::cache::*;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn cache_settings_execute_the_main_contract_without_platform_operations() {
    let directory = tempfile::tempdir().unwrap();
    let external_cache = tempfile::tempdir().unwrap();
    let server =
        crate::server::ContentServer::open_with_cache(directory.path(), external_cache.path())
            .unwrap();
    let native = server.native_graphql().unwrap();
    let mut events = server.runtime_state().events.subscribe();
    let cache = server.cache_paths().clone();
    std::fs::write(cache.root().join("scratch"), b"cached bytes").unwrap();
    let (_generation, mut requests) =
        crate::test_support::HostRequests::connect(&server.runtime_state().host);
    let response = native
        .execute(async_graphql::Request::new("{appCacheSize}"))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["appCacheSize"], 12);
    let response = native
        .execute(async_graphql::Request::new("mutation{clearAppCache}"))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["clearAppCache"], true);
    assert_eq!(size(&cache).unwrap(), 0);
    assert!(requests.try_recv().is_err());
    while let Ok(event) = events.try_recv() {
        assert_ne!(event.event_type, "CONTENT_CHANGED");
    }
    let state = server.runtime_state();
    let remote = crate::graphql::executor::execute_graphql(
        &state.public,
        async_graphql::Request::new("{appCacheSize}"),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(remote.errors.is_empty(), "{:?}", remote.errors);
    assert_eq!(remote.data.into_json().unwrap()["appCacheSize"], 0);
    server.shutdown().await;
}
