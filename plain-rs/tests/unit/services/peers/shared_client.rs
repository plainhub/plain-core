use super::*;
use plain_rs::db::DPeer;
use plain_rs::db::chat_store::SaveMode;
use plain_rs::db::chat_store::messages;
use plain_rs::db::chat_store::peers;
use plain_rs::files::shares::client::Link;
use plain_rs::services::peers::shared_download::download_url;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[path = "shared_client/fixtures.rs"]
mod fixtures;
use fixtures::*;

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
    let link = Link::new("localhost", 443, "fixture", &plain_rs::base64_encode(&[7;32])).unwrap();
    let url = link.file_url(&plain_rs::base64_encode(&[8;32]), "folder/file", false).unwrap();
    assert!(download_url(&url).is_ok());
    assert!(download_url("https://localhost/fs?id=g.AA").is_err());
    assert!(download_url("https://localhost/fs?id=c.client.AA").is_err());
    assert!(download_url("https://localhost/fs?id=x").is_err());
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
        plain_rs::services::files::remote_cache::RemoteCache::new(directory.path().join("cache"));
    let file = plain_rs::files::shares::client::File {
        name: "preview.txt".into(),
        relative_path: "folder/ +?#中文%".into(),
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
        relative_path: "folder/ +?#中文%".into(),
        is_dir: false,
        size: 8,
        mime_type: "image/jpeg".into(),
        has_thumb: true,
    };
    let cache =
        plain_rs::services::files::remote_cache::RemoteCache::new(directory.path().join("cache"));
    let old_key = json!({"host":link.host,"port":link.port,"shareId":link.shared_id,"token":link.token,"urlToken":url_token,"path":entry.relative_path,"size":entry.size}).to_string();
    let old_path = cache
        .download(
            &link
                .file_url(&url_token, &entry.relative_path, false)
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
