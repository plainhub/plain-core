use crate::server::ContentServer;
use serde_json::{Value, json};

#[tokio::test]
async fn native_directory_search_streams_unindexed_files_in_excluded_directories_and_finishes() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("node_modules");
    std::fs::create_dir(&source).unwrap();
    for index in 0..300 {
        std::fs::write(source.join(format!("needle-{index:03}.rs")), b"source").unwrap();
    }
    let server = ContentServer::open(&directory.path().join("core")).unwrap();
    let host = server.runtime_state().host.clone();
    let (generation, mut incoming) = host.connect();
    let permissions = tokio::spawn(async move {
        while let Some(request) = incoming.recv().await {
            assert_eq!(request["method"], "systemPermissionFacts");
            host.reply(
                generation,
                json!({"id":request["id"],"result":{"granted":{"WRITE_EXTERNAL_STORAGE":true}}}),
            )
            .unwrap();
        }
    });
    let executor = server.native_graphql().unwrap();
    let handle = executor.open_subscriptions(Default::default()).unwrap();
    let session = executor.subscription_session(handle).unwrap();
    session.send(&json!({"type":"subscribe","id":"search","payload":{
        "query":"subscription($root:String!){ fileSearchResults(root:$root,query:\"needle\",offset:0,limit:5,sortBy:NAME_ASC){items{name} count complete}}",
        "variables":{"root":directory.path().to_str().unwrap()}
    }}).to_string()).unwrap();
    let mut counts = Vec::new();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Ok(message) = session.read().await {
            let message: Value = serde_json::from_str(&message).unwrap();
            if message["type"] == "complete" {
                break;
            }
            assert_eq!(message["type"], "next", "{message}");
            if message["payload"]["extensions"]["subscriptionReady"] == true {
                continue;
            }
            let page = &message["payload"]["data"]["fileSearchResults"];
            counts.push(page["count"].as_i64().unwrap());
            if page["complete"] == true {
                assert_eq!(page["count"], 300);
                assert_eq!(page["items"][0]["name"], "needle-000.rs");
            } else {
                assert!(page["count"].as_i64().unwrap() < 300);
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(counts.len(), 3);
    executor.close_subscriptions(handle);
    server.shutdown().await;
    permissions.abort();
}
