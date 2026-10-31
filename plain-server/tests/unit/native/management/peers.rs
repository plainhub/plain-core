use crate::server::ContentServer;

#[tokio::test]
async fn native_reconnect_preserves_content_events_and_needs_no_host_request() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let mut events = state.events.subscribe();
    assert!(
        core.native_management()
            .unwrap()
            .reconnect_peers()
            .await
            .unwrap()
    );
    while let Ok(event) = events.try_recv() {
        assert_ne!(event.event_type, "CONTENT_CHANGED");
    }
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}
