use crate::graphql::executor::{RequestContext, execute_graphql};

async fn next_snapshot(
    stream: &mut futures_util::stream::BoxStream<'static, plain_graphql::Response>,
) -> plain_graphql::Response {
    use futures_util::StreamExt;
    tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
        .await
        .unwrap()
        .unwrap()
}

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
    let mut subscription = crate::graphql::executor::subscription_stream(
        state.clone(),
        plain_graphql::Request::new("subscription { httpServerChanged { remoteAccess } }"),
        RequestContext::authenticated_remote(state.clone(), "fixture".into()),
    );
    let denied = next_snapshot(&mut subscription).await;
    assert!(
        denied
            .errors
            .iter()
            .any(|error| error.message.contains("unsupported capability HTTP_SERVER")),
        "{:?}",
        denied.errors
    );
    drop(subscription);
    assert_eq!(state.http_server.snapshot().revision.0, 0);
    state.host.disconnect(generation);
    replies.await.unwrap();
    core.stop_core().await;
}

#[tokio::test]
async fn http_server_subscription_publishes_snapshots_only_after_changes_and_stops_on_drop() {
    use crate::graphql::executor::subscription_stream;
    use futures_util::StreamExt;
    use plain_libs::serde_json::json;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let replies = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!(["HTTP_SERVER"]),
                "onlineClientsChanged" | "httpServerStatusChanged" => json!(null),
                method => panic!("Unexpected host request: {method}"),
            };
            host.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    });
    let fields = "state {revision restarting httpPort httpsPort error} addresses onlineClients remoteAccess mdnsHostname";
    let query = execute_graphql(
        &state.public,
        plain_graphql::Request::new(format!("{{httpServer{{{fields}}}}}")),
        RequestContext::authenticated_remote(state.clone(), "fixture".into()),
    )
    .await;
    assert!(query.errors.is_empty(), "{:?}", query.errors);
    let mut stream = subscription_stream(
        state.clone(),
        plain_graphql::Request::new(format!("subscription{{httpServerChanged{{{fields}}}}}")),
        RequestContext::authenticated_remote(state.clone(), "fixture".into()),
    );
    let initial = next_snapshot(&mut stream).await;
    assert!(initial.errors.is_empty(), "{:?}", initial.errors);
    assert_eq!(
        initial.data.into_json().unwrap()["httpServerChanged"],
        query.data.into_json().unwrap()["httpServer"]
    );
    let client_session = state
        .subscription_sessions
        .open(
            state.clone(),
            RequestContext::authenticated_remote(state.clone(), "watch-fixture".into()),
            "watch-fixture".into(),
            None,
            Default::default(),
        )
        .unwrap();
    client_session.mark_online().unwrap();
    let joined = next_snapshot(&mut stream).await;
    assert!(joined.errors.is_empty(), "{:?}", joined.errors);
    assert_eq!(
        joined.data.into_json().unwrap()["httpServerChanged"]["onlineClients"],
        json!(["watch-fixture"])
    );
    client_session.close();
    let left = next_snapshot(&mut stream).await;
    assert!(left.errors.is_empty(), "{:?}", left.errors);
    assert_eq!(
        left.data.into_json().unwrap()["httpServerChanged"]["onlineClients"],
        json!([])
    );
    let receivers = state.events.receiver_count();
    assert!(futures_util::poll!(stream.next()).is_pending());
    state.http_server.started((8180, 8543));
    state.publish_http_status();
    let pushed = next_snapshot(&mut stream).await;
    assert!(pushed.errors.is_empty(), "{:?}", pushed.errors);
    assert_eq!(
        pushed.data.into_json().unwrap()["httpServerChanged"]["state"]["httpPort"],
        8180
    );
    assert!(futures_util::poll!(stream.next()).is_pending());
    drop(stream);
    assert_eq!(state.events.receiver_count(), receivers - 1);
    state.host.disconnect(generation);
    replies.await.unwrap();
    core.stop_core().await;
}
