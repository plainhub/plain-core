use super::*;
use crate::assembly::server::ContentServer;
use crate::services::peers::shared_download::download_url;
use plain_rs::db::DChat;
use plain_rs::db::DPeer;
use plain_rs::db::chat_store::SaveMode;
use plain_rs::db::chat_store::messages;
use plain_rs::db::chat_store::peers;
use plain_rs::files::shares::client::Link;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[path = "shared_client/fixtures.rs"]
mod fixtures;
use fixtures::*;

#[tokio::test]
async fn authenticated_main_browse_caches_and_queues_real_tls_files_without_host() {
    use crate::test_support::graphql_http_variables;
    let (directory, server, token) = root();
    let state = server.runtime_state();
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
async fn discovery_retry_rereads_peer_address_and_late_response_cannot_replace_card() {
    let (_dir, server, _) = root();
    let state = server.runtime_state();
    let (port, tls, count, requested, release) = tls().await;
    let expected = card(1);
    let row = save_card(&state, &expected);
    let peer = DPeer::new(
        "peer",
        "peer",
        "",
        Some(1),
        plain_rs::chat::enums::DeviceType::Phone,
    );
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let task = {
        let state = state.clone();
        let expected = expected.clone();
        let id = row.id.clone();
        tokio::spawn(async move { browse(&state, &id, &expected, None).await })
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut peer = peer;
    peer.ip = "invalid,127.0.0.1".into();
    peer.port = Some(port);
    peers::save(&state.db, &[peer], SaveMode::Update).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let result = result.unwrap();
    assert_eq!(result.link.port, port);
    let current = result.card;
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let task = {
        let state = state.clone();
        let id = row.id.clone();
        tokio::spawn(async move { browse(&state, &id, &current, Some("slow")).await })
    };
    tokio::time::timeout(Duration::from_secs(3), requested.notified())
        .await
        .unwrap();
    messages::content(
        &state.db,
        &row.id,
        "{\"type\":\"TEXT\",\"value\":{\"text\":\"replacement\"}}",
    )
    .unwrap();
    release.notify_one();
    assert!(task.await.unwrap().is_err());
    assert!(
        messages::get(&state.db, &row.id)
            .unwrap()
            .unwrap()
            .content
            .contains("replacement")
    );
    server.shutdown().await;
    tls.abort();
}
#[test]
fn discovery_selects_complete_matching_records_and_safe_ipv6_urls() {
    let expected = card(443);
    let mut links = vec![];
    let mut seen = HashSet::new();
    let snapshot = json!({"services":[{"complete":false,"port":8443,"txtRecords":["id=peer"],"ips":["127.0.0.1"]},{"complete":true,"port":8443,"txtRecords":["id=other"],"ips":["127.0.0.1"]},{"complete":true,"port":8443,"txtRecords":["id=peer"],"ips":["127.0.0.1","127.0.0.1"],"ipv6":["::1"]}]});
    discovery_candidates(&mut links, &mut seen, &snapshot, &expected);
    assert_eq!(links.len(), 2);
    assert!(links[1].page_url.starts_with("https://[::1]:8443/"));
    assert!(download_url("http://localhost/fs?sid=s&id=x").is_err());
    assert!(download_url("https://user@localhost/fs?sid=s&id=x").is_err());
    assert!(download_url("https://localhost/proxyfs?sid=s&id=x").is_err());
}

#[tokio::test]
async fn main_share_links_use_root_secret_ports_and_revocation_without_host() {
    let (dir, server, token) = root();
    let state = server.runtime_state();
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
#[tokio::test]
async fn root_candidates_preserve_foreign_self_unbound_and_deduplicated_endpoint_cases() {
    let (_dir, server, _) = root();
    let state = server.runtime_state();
    let mut expected = card(8443);
    expected.peer_info.ip = "10.0.0.9".into();
    let foreign = candidates(&state, &expected, false).unwrap();
    assert_eq!(foreign.len(), 1);
    assert_eq!(foreign[0].host, "10.0.0.9");
    assert_eq!(foreign[0].port, 8443);
    expected.peer_info.id = "local".into();
    state.prefs.set("https_port", 9999).unwrap();
    let own = candidates(&state, &expected, false).unwrap();
    assert_eq!(own[0].host, "127.0.0.1");
    assert_eq!(own[0].port, 9999);
    assert!(
        own.iter()
            .any(|link| link.host == "10.0.0.9" && link.port == 8443)
    );
    state.prefs.set("https_port", 0).unwrap();
    let unbound = candidates(&state, &expected, false).unwrap();
    assert_eq!(unbound.len(), 1);
    assert_eq!(unbound[0].host, "10.0.0.9");
    assert_eq!(unbound[0].port, 8443);
    state.prefs.set("https_port", 9999).unwrap();
    expected.peer_info.ip = "127.0.0.1".into();
    expected.peer_info.port = 9999;
    assert_eq!(
        candidates(&state, &expected, false)
            .unwrap()
            .iter()
            .filter(|link| link.host == "127.0.0.1" && link.port == 9999)
            .count(),
        1
    );
    server.shutdown().await;
}

#[tokio::test]
async fn stopping_the_core_cancels_an_inflight_encrypted_read_and_releases_capacity() {
    let (_directory, server, _) = root();
    let state = server.runtime_state();
    let (port, tls, count, requested, release) = tls().await;
    let expected = card(port);
    let row = save_card(&state, &expected);
    let read = {
        let state = state.clone();
        tokio::spawn(async move { browse(&state, &row.id, &expected, Some("slow")).await })
    };
    tokio::time::timeout(Duration::from_secs(3), requested.notified())
        .await
        .unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(state.lan.capacity.available_permits(), 3);
    server.stop_core().await;
    let error = tokio::time::timeout(Duration::from_secs(3), read)
        .await
        .unwrap()
        .unwrap()
        .err()
        .expect("stopped in-flight browse must fail");
    assert_eq!(error.to_string(), "Core stopped");
    assert_eq!(state.lan.capacity.available_permits(), 4);
    release.notify_one();
    server.shutdown().await;
    tls.abort();
}

#[tokio::test]
async fn stopped_core_rejects_every_typed_shared_operation_before_side_effects() {
    let (directory, server, _) = root();
    let state = server.runtime_state();
    let (port, tls, count, _, _) = tls().await;
    let expected = card(port);
    let row = save_card(&state, &expected);
    let link = Link::new(
        "127.0.0.1",
        port,
        "fixture",
        &plain_rs::base64_encode(&[7; 32]),
    )
    .unwrap();
    server.stop_core().await;
    let errors = [
        own_link(&state, "missing-share", None).await.err().unwrap(),
        initial_link(&state, &row.id, &expected)
            .await
            .err()
            .unwrap(),
        browse(&state, &row.id, &expected, None)
            .await
            .err()
            .unwrap(),
    ];
    for error in errors {
        assert_eq!(error.to_string(), "Core stopped");
    }
    let cache =
        crate::services::files::remote_cache::RemoteCache::new(directory.path().join("cache"));
    let file = plain_rs::files::shares::client::File {
        name: "preview.txt".into(),
        virtual_path: "folder/ +?#中文%".into(),
        is_dir: false,
        size: 8,
        mime_type: "text/plain".into(),
        has_thumb: false,
    };
    assert_eq!(
        cache_file(
            &state,
            &cache,
            link,
            plain_rs::base64_encode(&[8; 32]),
            file,
            false
        )
        .await
        .unwrap_err()
        .to_string(),
        "Download cancelled"
    );
    assert!(!directory.path().join("cache/remote_files").exists());
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert_eq!(state.lan.capacity.available_permits(), 4);
    server.shutdown().await;
    tls.abort();
}

#[tokio::test]
async fn thumbnail_requests_keep_aspect_ratio_and_replace_cropped_cache_entries() {
    let (directory, server, _) = root();
    let state = server.runtime_state();
    let (port, tls, _, _, _) = tls().await;
    let link = Link::new(
        "127.0.0.1",
        port,
        "fixture",
        &plain_rs::base64_encode(&[7; 32]),
    )
    .unwrap();
    let url_token = plain_rs::base64_encode(&[8; 32]);
    let entry = plain_rs::files::shares::client::File {
        name: "photo.jpg".into(),
        virtual_path: "folder/ +?#中文%".into(),
        is_dir: false,
        size: 8,
        mime_type: "image/jpeg".into(),
        has_thumb: true,
    };
    let cache =
        crate::services::files::remote_cache::RemoteCache::new(directory.path().join("cache"));
    let old_key = json!({"host":link.host,"port":link.port,"shareId":link.shared_id,"token":link.token,"urlToken":url_token,"path":entry.virtual_path,"size":entry.size}).to_string();
    let old_path = cache
        .download(
            &link
                .file_url(&url_token, &entry.virtual_path, false)
                .unwrap(),
            "thumbnail",
            Some(&old_key),
            state.stop.clone(),
            Some(&state.lan.client),
        )
        .await
        .unwrap();
    tokio::fs::write(&old_path, b"old cropped thumbnail")
        .await
        .unwrap();
    let first = cache_file(
        &state,
        &cache,
        link.clone(),
        url_token.clone(),
        entry.clone(),
        true,
    )
    .await
    .unwrap();
    assert_ne!(
        first, old_path,
        "Cropped cached thumbnails must not be reused"
    );
    assert_eq!(tokio::fs::read(&first).await.unwrap(), b"download");
    let second = cache_file(&state, &cache, link, url_token, entry, true)
        .await
        .unwrap();
    assert_eq!(
        first, second,
        "Proportional thumbnails keep a stable cache key"
    );
    assert_eq!(
        tokio::fs::read(&old_path).await.unwrap(),
        b"old cropped thumbnail"
    );
    server.shutdown().await;
    tls.abort();
}
