use crate::server::ContentServer;
use async_graphql::Request;
use serde_json::{Value, json};

async fn graphql(core: &ContentServer, query: &str, variables: Value) -> async_graphql::Response {
    core.native_graphql()
        .unwrap()
        .execute(Request::new(query).variables(async_graphql::Variables::from_json(variables)))
        .await
}

#[tokio::test]
async fn main_writes_preserve_utf8_metadata_and_refuse_overwrite_or_invalid_paths_without_host() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state().core;
    let (_generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    let parent = directory.path().join("parent/child");
    let response = graphql(
        &core,
        "mutation($path:String!){createDir(path:$path){isDir}}",
        json!({"path":parent}),
    )
    .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["createDir"]["isDir"],
        true
    );
    let path = parent.join("unicode.txt");
    let content = "fixture 中文\n";
    let query = "mutation($path:String!,$content:String!){writeTextFile(path:$path,content:$content,overwrite:false){name size updatedAt}}";
    let response = graphql(&core, query, json!({"path":path,"content":content})).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["writeTextFile"]["size"], content.len());
    assert_eq!(data["writeTextFile"]["name"], "unicode.txt");
    assert!(
        chrono::DateTime::parse_from_rfc3339(data["writeTextFile"]["updatedAt"].as_str().unwrap())
            .is_ok()
    );
    let response = graphql(&core, query, json!({"path":path,"content":"replacement"})).await;
    assert_eq!(response.errors.len(), 1);
    assert_eq!(std::fs::read_to_string(path).unwrap(), content);
    let response = graphql(
        &core,
        query,
        json!({"path":"relative-file.txt","content":"invalid"}),
    )
    .await;
    assert_eq!(response.errors[0].message, "absolute file path required");
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn shared_preview_export_recreates_the_rust_directory_after_cache_clear_without_platform_operations()
 {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state().core;
    let (_generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    let parent = core.cache_paths().shared_previews();
    assert!(parent.is_dir());
    let old = parent.join("old.txt");
    std::fs::write(&old, b"discard").unwrap();
    let response = graphql(&core, "mutation{clearAppCache}", json!({})).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(!parent.exists());
    let path = parent.join("shared.txt");
    let content = "shared\n\u{6587}\u{672c}\0";
    let response = graphql(
        &core,
        "mutation($directory:String!,$path:String!,$content:String!){createDir(path:$directory){path} writeTextFile(path:$path,content:$content,overwrite:true){path}}",
        json!({"directory":parent,"path":path,"content":content}),
    ).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(std::fs::read(&path).unwrap(), content.as_bytes());
    assert!(!old.exists());
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn text_export_replaces_only_the_selected_file_with_exact_captured_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state().core;
    let (_generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    let selected = directory.path().join("selected.txt");
    let neighbor = directory.path().join("neighbor.txt");
    std::fs::write(&selected, b"old content").unwrap();
    std::fs::write(&neighbor, b"untouched").unwrap();
    let content = "first\n\u{6d4b}\u{8bd5}\0last";
    let response = graphql(
        &core,
        "mutation($path:String!,$content:String!){writeTextFile(path:$path,content:$content,overwrite:true){path}}",
        json!({"path":selected,"content":content}),
    ).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["writeTextFile"]["path"],
        selected.to_string_lossy().as_ref()
    );
    assert_eq!(std::fs::read(selected).unwrap(), content.as_bytes());
    assert_eq!(std::fs::read(neighbor).unwrap(), b"untouched");
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn a_scan_failure_is_reported_after_the_physical_write_and_does_not_claim_a_result() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state().core;
    state.media_scans.close().unwrap();
    let path = directory.path().join("written.txt");
    let response = graphql(&core,
        "mutation($path:String!){writeTextFile(path:$path,content:\"completed\",overwrite:false){name}}", json!({"path":path})).await;
    assert_eq!(response.errors.len(), 1);
    assert!(response.errors[0].message.contains("Rust core stopped"));
    assert_eq!(std::fs::read(&path).unwrap(), b"completed");
    assert!(response.data.into_json().unwrap()["writeTextFile"].is_null());
    core.stop_core().await;
}
