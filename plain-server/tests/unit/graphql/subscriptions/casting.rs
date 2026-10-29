use crate::assembly::server::ContentServer;
use crate::native::graphql::NativeGraphqlExecutor;
use serde_json::{Value, json};

async fn snapshot(executor: &NativeGraphqlExecutor, handle: u64) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let message: Value = serde_json::from_str(
                &executor.subscription_session(handle).unwrap().read().await.unwrap(),
            ).unwrap();
            assert_eq!(message["type"], "next", "{message}");
            if message["payload"]["extensions"]["subscriptionReady"] == true { continue; }
            return message["payload"]["data"]["dlnaSenderChanged"].clone();
        }
    }).await.unwrap()
}

#[tokio::test]
async fn query_and_subscription_share_sender_state_without_starting_scan_or_changing_it() {
    let directory = tempfile::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let executor = server.native_graphql().unwrap();
    let fields = "version devices { id } items { path } playing progressMs durationMs";
    let before = state.cast.snapshot().version;
    let response = executor.execute(format!("query {{ dlnaSenderState {{ {fields} }} }}").into()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let queried = response.data.into_json().unwrap()["dlnaSenderState"].clone();
    assert_eq!(state.cast.snapshot().version, before);
    assert!(state.cast.scan.lock().unwrap().is_none());
    let handle = executor.open_subscriptions(Default::default()).unwrap();
    executor.subscription_session(handle).unwrap().send(&json!({
        "type":"subscribe", "id":"casting", "payload":{"query":format!("subscription {{ dlnaSenderChanged {{ {fields} }} }}")}
    }).to_string()).unwrap();
    assert_eq!(snapshot(&executor, handle).await, queried);
    let changed = executor.execute(r#"mutation { addDlnaItem(item:{path:"owned.mp3",title:"owned"}) { version items {path} } }"#.into()).await;
    assert!(changed.errors.is_empty(), "{:?}", changed.errors);
    let change = snapshot(&executor, handle).await;
    assert_eq!(change["version"], before + 1);
    assert_eq!(change["items"], json!([{"path":"owned.mp3"}]));
    assert!(state.cast.scan.lock().unwrap().is_none());
    executor.close_subscriptions(handle);
    server.shutdown().await;
}
