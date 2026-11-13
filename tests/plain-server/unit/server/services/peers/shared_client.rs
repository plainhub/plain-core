use crate::server::ContentServer;
use plain_libs::protocols::http_client;
use plain_libs::serde_json::Value;
use plain_libs::serde_json::json;
use plain_rs::features::chat::messages::share_card::Card;
use plain_rs::features::files::shares::client::Link;
use plain_rs::runtime::Runtime as ServerState;
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
    let client = http_client::builder().build().unwrap();
    let link = Link::new("127.0.0.1", port, "fixture", &expected.url_token).unwrap();
    let query = "query($root:String!){files(root:$root,query:\"\",offset:0,limit:10,sortBy:NAME_ASC){name path url size updatedAt}}";
    let variables = json!({"root":link.page_url});
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
    let file = &response["data"]["files"][0];
    assert_eq!(file["name"], "a 中文.txt");
    assert_eq!(file["size"], 8);
    assert_eq!(file["updatedAt"], "2026-01-01T00:00:00.000Z");
    let changed = plain_rs::infrastructure::db::chat_store::messages::get(&state.db, &row.id)
        .unwrap()
        .unwrap();
    let card: Value = plain_libs::serde_json::from_str(&changed.content).unwrap();
    assert_eq!(card["value"]["name"], "current");
    assert!(card["value"]["expiresAt"].is_null());
    let previews = directory.path().join("previews");
    let cached=graphql_http_variables(server.port,&token,"mutation($src:String!,$directory:String!){copyFileToDirectory(src:$src,directory:$directory,name:\"preview.txt\")}",json!({"src":file["url"],"directory":previews})).await;
    assert!(cached.get("errors").is_none(), "{cached}");
    assert_eq!(
        std::fs::read(cached["data"]["copyFileToDirectory"].as_str().unwrap()).unwrap(),
        b"download"
    );
    let repeated = graphql_http_variables(server.port, &token, query, variables).await;
    assert!(repeated.get("errors").is_none(), "{repeated}");
    let link = Link::new(
        "127.0.0.1",
        port,
        "fixture",
        &plain_libs::utils::base64::base64_encode_url_safe(&[7; 32]),
    )
    .unwrap();
    let target = directory.path().join("queued");
    let queued = graphql_http_variables(
        server.port,
        &token,
        "mutation($input: DownloadInput!) { createDownload(input: $input) { id } }",
        json!({"input":{"resources":{
            "urls":[format!("{}&action=download", link.file_url(&plain_libs::utils::base64::base64_encode(&[8;32]), "root", false).unwrap())],
            "targetDir":target.to_str().unwrap()
        }}}),
    ).await;
    assert!(queued.get("errors").is_none(), "{queued}");
    let id = queued["data"]["createDownload"]["id"].as_str().unwrap();
    let mut changed = state.downloads.changed.subscribe();
    let completed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let task = state
                .downloads
                .snapshot()
                .into_iter()
                .find(|task| task.id == id)
                .unwrap();
            if task.status.terminal() {
                return plain_libs::serde_json::to_value(task).unwrap();
            }
            changed.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(completed["status"], "COMPLETED", "{completed}");
    assert_eq!(completed["totalBytes"], 8);
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
async fn main_share_projection_contains_only_management_data_and_the_usable_url() {
    let (directory, server, token) = root();
    let state = server.runtime_state();
    let path = directory.path().join("file.txt");
    std::fs::write(&path, b"fixture").unwrap();
    let response=crate::test_support::graphql_http_variables(server.port,&token,
        "mutation($input:ShareInput!){createShare(input:$input){id name password readOnly expiresAt data{virtualPath realPath isDir} url}}",
        json!({"input":{"name":"fixture","password":"","readOnly":true,"data":[{"virtualPath":"/alias.txt","realPath":path,"isDir":false}]}})).await;
    assert!(response.get("errors").is_none(), "{response}");
    let row = &response["data"]["createShare"];
    assert_eq!(row["data"][0]["virtualPath"], "/alias.txt");
    let (link, path) = Link::parse(row["url"].as_str().unwrap()).unwrap();
    assert!(path.is_empty());
    let service =
        plain_rs::features::files::shares::Service::new(state.db.clone(), state.prefs.clone());
    assert_eq!(
        link.token,
        service.token(row["id"].as_str().unwrap()).unwrap()
    );
    server.shutdown().await;
}
