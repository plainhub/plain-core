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
            assert_eq!(request["method"], "systemAccessFacts");
            host.reply(
                generation,
                json!({"id":request["id"],"result":{"granted":{"FILES":true}}}),
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
    assert!(counts.len() >= 2);
    executor.close_subscriptions(handle);
    server.shutdown().await;
    permissions.abort();
}

#[tokio::test]
async fn large_search_keeps_frames_bounded_and_waits_for_a_slow_reader() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    std::fs::create_dir(&source).unwrap();
    for index in 0..10000 {
        std::fs::write(source.join(format!("needle-{index:05}.txt")), b"source").unwrap();
    }
    let server = ContentServer::open(&directory.path().join("core")).unwrap();
    let host = server.runtime_state().host.clone();
    let (generation, mut incoming) = host.connect();
    let permissions = tokio::spawn(async move {
        while let Some(request) = incoming.recv().await {
            host.reply(
                generation,
                json!({"id":request["id"],"result":{"granted":{"FILES":true}}}),
            )
            .unwrap();
        }
    });
    let executor = server.native_graphql().unwrap();
    let handle = executor.open_subscriptions(Default::default()).unwrap();
    let session = executor.subscription_session(handle).unwrap();
    session.send(&json!({"type":"subscribe","id":"search","payload":{
        "query":r#"subscription($root:String!){fileSearchResults(root:$root,query:"needle",offset:0,limit:10000,sortBy:NAME_ASC){items{name path size isDir createdAt updatedAt permission childCount} offset count complete}}"#,
        "variables":{"root":source.to_str().unwrap()}
    }}).to_string()).unwrap();
    let mut names = Vec::new();
    let mut final_page = false;
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        while let Ok(message) = session.read().await {
            assert!(message.len() < 1024 * 1024 - 49);
            let message: Value = serde_json::from_str(&message).unwrap();
            if message["type"] == "complete" {
                break;
            }
            assert_eq!(message["type"], "next", "{message}");
            if message["payload"]["extensions"]["subscriptionReady"] == true {
                continue;
            }
            let page = &message["payload"]["data"]["fileSearchResults"];
            let offset = page["offset"].as_i64().unwrap() as usize;
            if offset == 0 {
                names.clear();
            }
            assert_eq!(offset, names.len());
            let items = page["items"].as_array().unwrap();
            assert!(items.len() <= 128);
            names.extend(
                items
                    .iter()
                    .map(|item| item["name"].as_str().unwrap().to_owned()),
            );
            final_page |= page["complete"] == true;
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    assert!(final_page);
    assert_eq!(names.len(), 1000);
    assert_eq!(names.first().unwrap(), "needle-00000.txt");
    assert_eq!(names.last().unwrap(), "needle-00999.txt");
    executor.close_subscriptions(handle);
    server.shutdown().await;
    permissions.abort();
}

#[test]
fn long_paths_split_by_byte_budget_before_the_record_count_limit() {
    let page = plain_libs::filesystem::browse::Page {
        count: 20,
        items: (0..20)
            .map(|index| plain_libs::filesystem::record::FileRecord {
                name: format!("needle-{index:03}"),
                path: format!("/{}-{index}", "segment/".repeat(4000)),
                permission: String::new(),
                created_at: None,
                updated_at: 0,
                size: 0,
                is_dir: false,
                child_count: 0,
                file_id: String::new(),
            })
            .collect(),
    };
    let batches = super::batches(page, true);
    assert_eq!(batches.len(), 3);
    let mut offset = 0;
    for (index, batch) in batches.into_iter().enumerate() {
        let batch = batch.unwrap();
        assert_eq!(batch.offset.0, offset);
        assert_eq!(batch.count.0, 20);
        assert_eq!(batch.complete, index == 2);
        assert!(batch.items.len() <= 8);
        offset += batch.items.len() as i64;
    }
    assert_eq!(offset, 20);
}
