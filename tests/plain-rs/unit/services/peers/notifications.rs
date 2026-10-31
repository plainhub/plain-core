use anyhow::Result;
use plain_rs::chat::enums::DeviceType;
use plain_rs::db::DPeer;
use plain_rs::services::notifications::types::Notification;
use plain_rs::services::peers::management::Identity;
use serde_json::json;
use std::collections::HashMap;

use super::*;
use plain_rs::services::peers::metadata::AppSnapshot as App;
use plain_rs::types::Instant;
use std::sync::Arc;
use std::time::Duration;

struct CallbackReceiver {
    host: Arc<plain_rs::host::bridge::Host>,
    generation: u64,
    requests: tokio::sync::mpsc::Receiver<serde_json::Value>,
    buffered: std::collections::VecDeque<serde_json::Value>,
}

impl CallbackReceiver {
    fn new(host: Arc<plain_rs::host::bridge::Host>) -> Self {
        let (generation, requests) = host.connect();
        Self {
            host,
            generation,
            requests,
            buffered: Default::default(),
        }
    }

    fn try_recv(&mut self) -> Result<serde_json::Value, tokio::sync::mpsc::error::TryRecvError> {
        if let Some(value) = self.buffered.pop_front() {
            return Ok(value);
        }
        loop {
            let request = self.requests.try_recv()?;
            self.host
                .reply(self.generation, json!({"id":request["id"],"result":null}))
                .unwrap();
            if request["method"] == "peerNotificationsChanged" {
                return Ok(request["params"].clone());
            }
        }
    }

    async fn recv(&mut self) -> Option<serde_json::Value> {
        if let Some(value) = self.buffered.pop_front() {
            return Some(value);
        }
        loop {
            let request = self.requests.recv().await?;
            self.host
                .reply(self.generation, json!({"id":request["id"],"result":null}))
                .unwrap();
            if request["method"] == "peerNotificationsChanged" {
                return Some(request["params"].clone());
            }
        }
    }
}

fn fixture() -> (
    Store,
    DPeer,
    Identity,
    Arc<plain_rs::host::bridge::Host>,
    CallbackReceiver,
) {
    let peer = DPeer::new(
        "peer",
        "Fixture",
        "127.0.0.1",
        Some(18443),
        DeviceType::Computer,
    );
    let identity = Identity {
        url: "wss://127.0.0.1:18443/?cid=fixture".into(),
        token: plain_rs::utils::base64::base64_encode(&[7; 32]),
        ip: peer.ip.clone(),
        port: peer.port,
    };
    let host = Arc::new(plain_rs::host::bridge::Host::default());
    let receiver = CallbackReceiver::new(host.clone());
    let store = Store::default();
    store.reconcile(
        std::slice::from_ref(&peer),
        &HashMap::from([(peer.id.clone(), identity.clone())]),
        &host,
    );
    (store, peer, identity, host, receiver)
}

fn notification(id: &str) -> Notification {
    Notification {
        id: id.into(),
        only_once: false,
        is_clearable: true,
        app_id: "fixture.app".into(),
        app_name: "Fixture".into(),
        posted_at: Instant(chrono::DateTime::from_timestamp(0, 0).unwrap()),
        silent: false,
        title: id.into(),
        body: "Body".into(),
        actions: vec![],
        reply_actions: vec![],
    }
}

fn observation(items: Vec<Notification>) -> Result<(App, Vec<Notification>, reqwest::Url)> {
    Ok((
        App {
            device_name: "Fixture".into(),
            capabilities: vec!["NOTIFICATIONS".into()],
            permissions: vec!["NOTIFICATION_LISTENER".into()],
            url_token: plain_rs::utils::base64::base64_encode(&[9; 32]),
        },
        items,
        reqwest::Url::parse("https://127.0.0.1:18443").unwrap(),
    ))
}

fn event(receiver: &mut CallbackReceiver) -> serde_json::Value {
    receiver.try_recv().unwrap()
}

#[tokio::test]
async fn baseline_diffs_and_repeated_hints_only_alert_new_confirmed_items() {
    let (store, peer, identity, events, mut receiver) = fixture();
    assert!(event(&mut receiver)["group"]["loading"].as_bool().unwrap());
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("old")]),
            &events,
        )
        .unwrap();
    let baseline = event(&mut receiver);
    assert!(baseline["alerts"].as_array().unwrap().is_empty());
    assert_eq!(baseline["group"]["deviceType"], "COMPUTER");
    let icon = baseline["group"]["items"][0]["icon"].clone();
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("old")]),
            &events,
        )
        .unwrap();
    assert!(receiver.try_recv().is_err());
    let mut updated = notification("old");
    updated.body = "Updated".into();
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("new"), updated]),
            &events,
        )
        .unwrap();
    let update = event(&mut receiver);
    assert_eq!(update["alerts"].as_array().unwrap().len(), 1);
    assert_eq!(update["alerts"][0]["id"], "new");
    assert_eq!(update["group"]["items"][0]["icon"], icon);
    assert_eq!(update["group"]["items"][1]["body"], "Updated");
    store
        .apply("local", &peer.id, &identity, observation(vec![]), &events)
        .unwrap();
    assert!(
        event(&mut receiver)["group"]["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn silent_items_and_active_updates_do_not_alert_but_a_dismissed_id_can_notify_again() {
    let (store, peer, identity, events, mut receiver) = fixture();
    event(&mut receiver);
    let mut once = notification("once");
    once.only_once = true;
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![once.clone()]),
            &events,
        )
        .unwrap();
    event(&mut receiver);
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            Err(anyhow::anyhow!("offline")),
            &events,
        )
        .unwrap();
    let offline = event(&mut receiver);
    assert_eq!(offline["group"]["online"], false);
    assert_eq!(offline["group"]["items"][0]["id"], "once");
    once.body = "Updated active notification".into();
    let mut silent = notification("silent");
    silent.silent = true;
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![once.clone(), silent]),
            &events,
        )
        .unwrap();
    assert!(
        event(&mut receiver)["alerts"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    store
        .apply("local", &peer.id, &identity, observation(vec![]), &events)
        .unwrap();
    event(&mut receiver);
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![once]),
            &events,
        )
        .unwrap();
    assert_eq!(event(&mut receiver)["alerts"][0]["id"], "once");
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("ordinary")]),
            &events,
        )
        .unwrap();
    assert_eq!(event(&mut receiver)["alerts"][0]["id"], "ordinary");
}

#[tokio::test]
async fn capability_or_permission_revocation_clears_data_and_regrant_starts_a_fresh_baseline() {
    for capability in [true, false] {
        let (store, peer, identity, events, mut receiver) = fixture();
        event(&mut receiver);
        store
            .apply(
                "local",
                &peer.id,
                &identity,
                observation(vec![notification("old")]),
                &events,
            )
            .unwrap();
        event(&mut receiver);
        let (mut app, _, base) = observation(vec![]).unwrap();
        if capability {
            app.capabilities.clear()
        } else {
            app.permissions.clear()
        }
        store
            .apply(
                "local",
                &peer.id,
                &identity,
                Ok((app, vec![], base)),
                &events,
            )
            .unwrap();
        let revoked = event(&mut receiver);
        assert!(revoked["group"]["items"].as_array().unwrap().is_empty());
        assert_eq!(revoked["group"]["online"], true);
        store
            .apply(
                "local",
                &peer.id,
                &identity,
                observation(vec![notification("after-regrant")]),
                &events,
            )
            .unwrap();
        assert!(
            event(&mut receiver)["alerts"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[tokio::test]
async fn snapshots_recover_without_alerts_and_credentials_or_membership_retire_old_results() {
    let (store, mut peer, identity, events, mut receiver) = fixture();
    event(&mut receiver);
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("old")]),
            &events,
        )
        .unwrap();
    event(&mut receiver);
    let initial: serde_json::Value = store.snapshot();
    assert_eq!(initial["groups"][0]["items"][0]["id"], "old");
    assert!(initial.get("alerts").is_none());
    peer.name = "Renamed".into();
    store.reconcile(
        std::slice::from_ref(&peer),
        &HashMap::from([(peer.id.clone(), identity.clone())]),
        &events,
    );
    let renamed = event(&mut receiver);
    assert_eq!(renamed["group"]["name"], "Renamed");
    assert_eq!(renamed["group"]["items"][0]["id"], "old");
    let mut moved = identity.clone();
    moved.ip = "127.0.0.2".into();
    moved.url = "wss://127.0.0.2:18443/?cid=fixture".into();
    store.reconcile(
        std::slice::from_ref(&peer),
        &HashMap::from([(peer.id.clone(), moved.clone())]),
        &events,
    );
    assert!(receiver.try_recv().is_err());
    let retained: serde_json::Value = store.snapshot();
    assert_eq!(retained["groups"][0]["items"][0]["id"], "old");
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("stale-address")]),
            &events,
        )
        .unwrap();
    assert!(receiver.try_recv().is_err());
    let (app, items, _) =
        observation(vec![notification("old"), notification("fresh-after-move")]).unwrap();
    store
        .apply(
            "local",
            &peer.id,
            &moved,
            Ok((
                app,
                items,
                reqwest::Url::parse("https://127.0.0.2:18443").unwrap(),
            )),
            &events,
        )
        .unwrap();
    let moved_event = event(&mut receiver);
    assert_eq!(moved_event["alerts"][0]["id"], "fresh-after-move");
    assert!(
        moved_event["group"]["items"][0]["icon"]
            .as_str()
            .unwrap()
            .starts_with("https://127.0.0.2:18443/")
    );
    let mut replacement = moved.clone();
    replacement.token = plain_rs::utils::base64::base64_encode(&[8; 32]);
    store.reconcile(
        std::slice::from_ref(&peer),
        &HashMap::from([(peer.id.clone(), replacement)]),
        &events,
    );
    assert!(
        event(&mut receiver)["group"]["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("stale")]),
            &events,
        )
        .unwrap();
    assert!(receiver.try_recv().is_err());
    store.reconcile(&[], &HashMap::new(), &events);
    assert!(event(&mut receiver)["group"].is_null());
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("removed")]),
            &events,
        )
        .unwrap();
    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn invalid_snapshots_do_not_mutate_projection_and_icons_decrypt_with_standard_url_token() {
    let (store, peer, identity, events, mut receiver) = fixture();
    event(&mut receiver);
    let duplicate = notification("duplicate");
    assert!(
        store
            .apply(
                "local",
                &peer.id,
                &identity,
                observation(vec![duplicate.clone(), duplicate]),
                &events
            )
            .is_err()
    );
    assert!(receiver.try_recv().is_err());
    let (mut app, items, base) = observation(vec![notification("valid")]).unwrap();
    app.url_token = plain_rs::utils::base64::base64_encode(&[9; 31]);
    assert!(
        store
            .apply(
                "local",
                &peer.id,
                &identity,
                Ok((app, items, base)),
                &events
            )
            .is_err()
    );
    assert!(receiver.try_recv().is_err());
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("valid")]),
            &events,
        )
        .unwrap();
    let value = event(&mut receiver);
    let url = reqwest::Url::parse(value["group"]["items"][0]["icon"].as_str().unwrap()).unwrap();
    let file_id = url
        .query_pairs()
        .find(|(key, _)| key == "id")
        .unwrap()
        .1
        .into_owned();
    let (scope, payload) = plain_rs::files::resource_link::parse(&file_id).unwrap();
    assert_eq!(
        scope,
        plain_rs::files::resource_link::Scope::Session("local".into())
    );
    let resource = plain_rs::files::resource_link::open(&scope, &[9; 32], payload).unwrap();
    assert_eq!(
        resource,
        plain_rs::files::resource_link::Resource::reference("pkgicon://fixture.app")
    );
    assert!(value["alerts"].as_array().unwrap().is_empty());
    assert!(!store.snapshot().to_string().contains("urlToken"));
}

#[tokio::test]
async fn confirmed_authorization_failures_clear_data_instead_of_looking_like_network_timeouts() {
    use plain_rs::services::peers::management_http::AuthorizationError;
    for permission in [true, false] {
        let (store, peer, identity, events, mut receiver) = fixture();
        event(&mut receiver);
        store
            .apply(
                "local",
                &peer.id,
                &identity,
                observation(vec![notification("old")]),
                &events,
            )
            .unwrap();
        event(&mut receiver);
        let error = if permission {
            AuthorizationError::Permission
        } else {
            AuthorizationError::Session
        };
        store
            .apply("local", &peer.id, &identity, Err(error.into()), &events)
            .unwrap();
        let denied = event(&mut receiver);
        assert_eq!(denied["group"]["online"], permission);
        assert!(denied["group"]["items"].as_array().unwrap().is_empty());
        store
            .apply(
                "local",
                &peer.id,
                &identity,
                observation(vec![notification("after-login")]),
                &events,
            )
            .unwrap();
        assert!(
            event(&mut receiver)["alerts"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[tokio::test]
async fn notifications_arriving_after_snapshot_retain_their_host_callback() {
    let (store, peer, identity, host, mut callbacks) = fixture();
    event(&mut callbacks);
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("old")]),
            &host,
        )
        .unwrap();
    event(&mut callbacks);
    let initial = store.snapshot();
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("new"), notification("old")]),
            &host,
        )
        .unwrap();
    let update = event(&mut callbacks);
    assert_eq!(initial["groups"][0]["items"][0]["id"], "old");
    assert!(update["revision"].as_u64().unwrap() > initial["revision"].as_u64().unwrap());
    assert_eq!(update["alerts"][0]["id"], "new");
}

#[tokio::test]
async fn reconnecting_host_receives_a_complete_notification_snapshot_without_alerts() {
    let (store, peer, identity, host, mut old) = fixture();
    event(&mut old);
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("old")]),
            &host,
        )
        .unwrap();
    event(&mut old);
    let mut replacement = CallbackReceiver::new(host.clone());
    store.notify_snapshot(&host).unwrap();
    let snapshot = event(&mut replacement);
    assert_eq!(snapshot["groups"][0]["items"][0]["id"], "old");
    assert!(snapshot.get("alerts").is_none());
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("new")]),
            &host,
        )
        .unwrap();
    let update = event(&mut replacement);
    assert!(update["revision"].as_u64().unwrap() > snapshot["revision"].as_u64().unwrap());
    assert_eq!(update["alerts"][0]["id"], "new");
}

#[tokio::test]
async fn saturated_host_recovers_the_latest_snapshot_and_missed_alerts() {
    let (store, peer, identity, host, mut callbacks) = fixture();
    event(&mut callbacks);
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("old")]),
            &host,
        )
        .unwrap();
    event(&mut callbacks);
    for _ in 0..plain_rs::host::bridge::REQUEST_CAPACITY {
        host.notify("blocked", json!({})).unwrap();
    }
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("new")]),
            &host,
        )
        .unwrap();
    assert!(callbacks.try_recv().is_err());
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("latest")]),
            &host,
        )
        .unwrap();
    let recovery = tokio::time::timeout(Duration::from_secs(3), callbacks.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovery["groups"][0]["items"][0]["id"], "latest");
    assert_eq!(
        recovery["alerts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["new", "latest"]
    );
}
#[tokio::test]
async fn state_changes_before_host_attachment_are_delivered_as_the_initial_snapshot() {
    let (store, peer, identity, host, mut callbacks) = fixture();
    event(&mut callbacks);
    host.disconnect(callbacks.generation);
    drop(callbacks);
    let before = host
        .callback_attempts
        .load(std::sync::atomic::Ordering::Relaxed);
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("before-attach")]),
            &host,
        )
        .unwrap();
    assert_eq!(
        host.callback_attempts
            .load(std::sync::atomic::Ordering::Relaxed),
        before
    );
    let mut current = CallbackReceiver::new(host.clone());
    store.notify_snapshot(&host).unwrap();
    let snapshot = event(&mut current);
    assert_eq!(snapshot["groups"][0]["items"][0]["id"], "before-attach");
    assert!(snapshot.get("alerts").is_none());
    store
        .apply(
            "local",
            &peer.id,
            &identity,
            observation(vec![notification("after-attach")]),
            &host,
        )
        .unwrap();
    let update = event(&mut current);
    assert_eq!(update["alerts"][0]["id"], "after-attach");
    assert!(update["revision"].as_u64().unwrap() > snapshot["revision"].as_u64().unwrap());
    host.disconnect(current.generation);
}
