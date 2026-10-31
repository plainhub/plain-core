use crate::server::ContentServer;
use plain_rs::chat::messages::share_card::Card;
use plain_rs::files::shares::client::Link;
use plain_rs::runtime::Runtime as ServerState;
use serde_json::Value;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
#[path = "shared_client/fixtures.rs"]
mod fixtures;
use fixtures::*;

#[tokio::test]
async fn authenticated_main_browse_caches_and_queues_real_tls_files_without_host() {
    use crate::test_support::graphql_http_variables;
    let (directory, server, token) = root();
    let state = server.runtime_state().core;
    let (port, tls, _, _, _) = tls().await;
    let expected = card(port);
    let row = save_card(&state, &expected);
    let client = reqwest::Client::new();
    let query = "mutation($id:ID!,$expected:SharedMessageCardInput!){browseSharedLink(messageId:$id,expected:$expected){info{urlToken} link{host port sharedId token pageUrl} card{name expiresAt totalSize}}}";
    let variables = json!({"id":row.id,"expected":expected});
    assert_eq!(
        client
            .post(format!("http://127.0.0.1:{}/graphql", server.port))
            .header("content-type", "application/json")
            .body(json!({"query":query,"variables":variables}).to_string())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let response = graphql_http_variables(server.port, &token, query, variables.clone()).await;
    assert!(response.get("errors").is_none(), "{response}");
    let result = &response["data"]["browseSharedLink"];
    assert_eq!(result["card"]["name"], "current");
    assert!(result["card"]["expiresAt"].is_null());
    assert_eq!(result["card"]["totalSize"], 8);
    let link: Link = serde_json::from_value(result["link"].clone()).unwrap();
    let cached = graphql_http_variables(server.port, &token,
        "mutation($link:SharedLinkInput!,$urlToken:String!,$entry:SharedEntryInput!){cacheSharedFile(link:$link,urlToken:$urlToken,entry:$entry,thumbnail:false)}",
        json!({"link":link,"urlToken":result["info"]["urlToken"],"entry":{"name":"preview.txt","virtualPath":"folder/ +?#中文%","isDir":false,"size":8,"mimeType":"text/plain","hasThumb":false}})).await;
    assert!(cached.get("errors").is_none(), "{cached}");
    assert_eq!(
        tokio::fs::read(cached["data"]["cacheSharedFile"].as_str().unwrap())
            .await
            .unwrap(),
        b"download"
    );
    assert_eq!(state.lan.capacity.available_permits(), 4);
    let stale = graphql_http_variables(server.port, &token, query, variables).await;
    assert!(stale.get("errors").is_some(), "{stale}");
    let link = Link::new(
        "127.0.0.1",
        port,
        "fixture",
        &plain_rs::utils::base64::base64_encode_url_safe(&[7; 32]),
    )
    .unwrap();
    let target = directory.path().join("queued");
    let queued = graphql_http_variables(
        server.port,
        &token,
        "mutation($input: SharedDownloadInput!) { enqueueSharedDownload(input: $input) }",
        json!({"input":{
            "messageId":row.id,"kind":"SYNC","link":link,
            "urlToken":result["info"]["urlToken"],
            "entries":[{"name":"Root","virtualPath":"root","isDir":true,"size":0,"mimeType":"","hasThumb":false}],
            "targetDir":target.to_str().unwrap(),"zipName":""
        }}),
    ).await;
    assert!(queued.get("errors").is_none(), "{queued}");
    let id = queued["data"]["enqueueSharedDownload"].as_str().unwrap();
    let mut changed = state.shared_batches.queue.changed.subscribe();
    let completed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = state.shared_batches.queue.snapshot();
            let task = snapshot["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|task| task["id"] == id)
                .unwrap();
            if matches!(
                task["status"].as_str(),
                Some("COMPLETED" | "PARTIAL" | "FAILED" | "CANCELED")
            ) {
                return task.clone();
            }
            changed.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(completed["status"], "COMPLETED", "{completed}");
    assert_eq!(completed["totalSize"], 8);
    assert_eq!(completed["totalFiles"], 1);
    assert_eq!(completed["doneFiles"], 1);
    assert_eq!(
        tokio::fs::read(target.join("Root/a 中文.txt"))
            .await
            .unwrap(),
        b"download"
    );
    assert!(!state.host.connected());
    server.shutdown().await;
    tls.abort();
}

#[tokio::test]
async fn main_share_links_use_root_secret_ports_and_revocation_without_host() {
    let (dir, server, token) = root();
    let state = server.runtime_state().core;
    state
        .prefs
        .set(
            "master_secret",
            plain_rs::utils::base64::base64_encode(&[5; 32]),
        )
        .unwrap();
    state.prefs.set("https_port", 2443).unwrap();
    let path = dir.path().join("file.txt");
    std::fs::write(&path, b"fixture").unwrap();
    let service = plain_rs::files::shares::Service::new(state.db.clone(), state.prefs.clone());
    let share = service
        .create(
            "fixture".into(),
            vec![path.to_string_lossy().into_owned()],
            true,
            None,
        )
        .unwrap();
    let query = r#"query($id:ID!){shareLink(id:$id,host:"::1"){token pageUrl}}"#;
    let variables = json!({"id":share.id});
    let response =
        crate::test_support::graphql_http_variables(server.port, &token, query, variables.clone())
            .await;
    assert!(response.get("errors").is_none(), "{response}");
    assert_eq!(
        response["data"]["shareLink"]["token"],
        service.token(&share.id).unwrap()
    );
    assert!(
        response["data"]["shareLink"]["pageUrl"]
            .as_str()
            .unwrap()
            .starts_with("https://[::1]:2443/s/")
    );
    state.db.share_delete(&share.id).unwrap();
    let response =
        crate::test_support::graphql_http_variables(server.port, &token, query, variables).await;
    assert!(response.get("errors").is_some(), "{response}");
    assert!(!state.host.connected());
    server.shutdown().await;
}
