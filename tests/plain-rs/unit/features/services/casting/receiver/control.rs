use super::*;
use crate::test_runtime::Core as ContentServer;

#[tokio::test]
async fn renderer_state_is_sent_to_the_host_without_a_broadcast_event() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open_with_cache(
        &directory.path().join("data"),
        &directory.path().join("cache"),
    )
    .unwrap();
    let state = core.runtime_state();
    let (generation, mut requests) = state.host.connect();
    let mut events = state.events.subscribe();
    let mut renderer = DlnaRendererState::default();
    renderer.media_uri = "https://example.test/media".into();
    emit(&state, &renderer);
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "dlnaRendererChanged");
    assert_eq!(request["params"]["mediaUri"], renderer.media_uri);
    state
        .host
        .reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    assert!(events.try_recv().is_err());
    state.host.disconnect(generation);
    core.stop_core().await;
}
