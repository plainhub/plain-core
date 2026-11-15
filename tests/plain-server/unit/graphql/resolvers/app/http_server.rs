use super::*;
use crate::graphql::executor::{RequestContext, execute_graphql};

#[tokio::test]
async fn other_hosts_and_unauthenticated_callers_cannot_manage_the_http_server() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let replies = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            assert_eq!(request["method"], "systemCapabilities");
            host.reply(
                generation,
                plain_libs::serde_json::json!({"id":request["id"],"result":[]}),
            )
            .unwrap();
        }
    });
    let unauthenticated = execute_graphql(
        &state.public,
        plain_graphql::Request::new("{httpServer{remoteAccess}}"),
        RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!unauthenticated.errors.is_empty());
    for document in [
        "{httpServer{remoteAccess}}",
        "mutation{restartHttpServer(httpPort:0){revision}}",
        "mutation{acquireMdnsDebugSession(token:\"fixture\")}",
        "mutation{releaseMdnsDebugSession(token:\"fixture\")}",
    ] {
        let response = execute_graphql(
            &state.public,
            plain_graphql::Request::new(document),
            RequestContext::authenticated_remote(state.clone(), "fixture".into()),
        )
        .await;
        assert!(
            response
                .errors
                .iter()
                .any(|error| error.message.contains("unsupported capability HTTP_SERVER")),
            "{:?}",
            response.errors
        );
    }
    assert_eq!(state.http_server.snapshot().revision.0, 0);
    state.host.disconnect(generation);
    replies.await.unwrap();
    core.stop_core().await;
}
