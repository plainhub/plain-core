use super::*;
use crate::features::files::shares::client::Link;
use crate::features::services::downloads::resources::url as download_url;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::chat_store::SaveMode;
use crate::infrastructure::db::chat_store::messages;
use crate::infrastructure::db::chat_store::peers;
use plain_libs::serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[path = "shared_client/fixtures.rs"]
mod fixtures;
use fixtures::*;

#[tokio::test]
async fn discovery_selects_complete_matching_records_and_safe_ipv6_urls() {
    let (_directory, core, _) = root();
    let state = core.runtime_state();
    let expected = card(443);
    let mut links = vec![];
    let mut seen = HashSet::new();
    let snapshot = json!({"services":[{"complete":false,"port":8443,"txtRecords":["id=peer"],"ips":["127.0.0.1"]},{"complete":true,"port":8443,"txtRecords":["id=other"],"ips":["127.0.0.1"]},{"complete":true,"port":8443,"txtRecords":["id=peer"],"ips":["127.0.0.1","127.0.0.1"],"ip6s":["::1"]}]});
    discovery_candidates(&mut links, &mut seen, &snapshot, &expected);
    assert_eq!(links.len(), 2);
    assert!(links[1].page_url.starts_with("https://[::1]:8443/"));
    assert!(download_url(&state, "http://localhost/fs?sid=s&id=x").is_err());
    assert!(download_url(&state, "https://user@localhost/fs?sid=s&id=x").is_err());
    assert!(download_url(&state, "https://localhost/proxyfs?sid=s&id=x").is_err());
    let link = Link::new(
        "localhost",
        443,
        "fixture",
        &plain_libs::utils::base64::base64_encode(&[7; 32]),
    )
    .unwrap();
    let url = link
        .file_url(
            &plain_libs::utils::base64::base64_encode(&[8; 32]),
            "folder/file",
            false,
        )
        .unwrap();
    assert!(download_url(&state, &format!("{url}&action=download")).is_ok());
    assert!(download_url(&state, &format!("{url}&action=preview")).is_err());
    assert!(download_url(&state, "https://localhost/fs?id=c.owner.AA").is_err());
    assert!(download_url(&state, "https://localhost/fs?id=c.client.AA").is_err());
    assert!(download_url(&state, "https://localhost/fs?id=x").is_err());
    core.shutdown().await;
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
        &plain_libs::utils::base64::base64_encode(&[7; 32]),
    )
    .unwrap();
    server.stop_core().await;
    let errors = [
        own_link(&state, "missing-share").await.err().unwrap(),
        browse(&state, &row.id, &expected, None)
            .await
            .err()
            .unwrap(),
    ];
    for error in errors {
        assert_eq!(error.to_string(), "Core stopped");
    }
    assert!(
        files(&state, file_request(link.page_url))
            .await
            .unwrap_err()
            .to_string()
            .contains("Core stopped")
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
        crate::features::chat::enums::DeviceType::Phone,
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
    peer.ip = "127.0.0.1".into();
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

fn file_request(root: String) -> plain_libs::filesystem::browse::Request {
    plain_libs::filesystem::browse::Request {
        root,
        query: String::new(),
        sort_by: plain_libs::filesystem::SortBy::NameAsc,
        offset: 0,
        limit: None,
        count_only: false,
        count_children: false,
    }
}

#[tokio::test]
async fn generic_preview_copy_reuses_named_thumbnail_bytes_without_reusing_obsolete_versions() {
    let (directory, server, _) = root();
    let state = server.runtime_state();
    let (port, tls, count, _, _) = tls().await;
    let link = Link::new(
        "127.0.0.1",
        port,
        "fixture",
        &plain_libs::utils::base64::base64_encode(&[7; 32]),
    )
    .unwrap();
    let url = link
        .file_url(
            &plain_libs::utils::base64::base64_encode(&[8; 32]),
            "folder/ +?#中文%",
            false,
        )
        .unwrap();
    let target = directory.path().join("previews");
    let copy = |version: u8| {
        let state = state.clone();
        let target = target.to_string_lossy().into_owned();
        let source = format!("{url}&action=preview&size=sm&v={version}");
        async move {
            crate::features::services::files::named_copy::copy(
                &state,
                "local".into(),
                source,
                target,
                "thumbnail".into(),
            )
            .await
            .unwrap()
        }
    };
    let first = copy(1).await;
    let second = copy(1).await;
    assert_eq!(std::fs::read(&first).unwrap(), b"download");
    assert_eq!(std::fs::read(&second).unwrap(), b"download");
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let third = copy(2).await;
    assert_eq!(std::fs::read(third).unwrap(), b"download");
    assert_eq!(count.load(Ordering::SeqCst), 2);
    server.shutdown().await;
    tls.abort();
}
