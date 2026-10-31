use super::*;
use plain_rs::host::startup::StartupConfig;

fn startup_json() -> Value {
    json!({
        "debug": false, "name": "fixture", "deviceType": "COMPUTER",
        "buildChannel": "GITHUB", "downloadsDir": "", "manufacturer": "",
        "model": "", "osName": "", "osVersion": "", "appVersion": "",
        "appBuildNumber": "", "cpuArch": "", "wifiAwareSupported": false,
        "recentFilesCatalog": false, "systemMediaCatalog": false, "supportedAbis": []
    })
}

#[tokio::test]
async fn callbacks_enter_the_host_queue_in_order_and_release_capacity_on_reply() {
    let host = Host::default();
    let (generation, mut incoming) = host.connect();
    host.notify("systemWebLoginClosed", json!({"requestId":"old"}))
        .unwrap();
    host.notify("systemWebLoginRequested", json!({"requestId":"new"}))
        .unwrap();
    let first = incoming.recv().await.unwrap();
    let second = incoming.recv().await.unwrap();
    assert_eq!(first["method"], "systemWebLoginClosed");
    assert_eq!(first["params"]["requestId"], "old");
    assert_eq!(second["method"], "systemWebLoginRequested");
    assert_eq!(second["params"]["requestId"], "new");
    host.reply(generation, json!({"id":first["id"],"result":null}))
        .unwrap();
    host.reply(generation, json!({"id":second["id"],"result":null}))
        .unwrap();
    tokio::task::yield_now().await;
    assert!(host.state.lock().unwrap().pending.is_empty());
    assert_eq!(host.capacity.available_permits(), REQUEST_CAPACITY);
    host.disconnect(generation);
    assert_eq!(
        host.notify("systemWebLoginClosed", json!({})).unwrap_err(),
        "host unavailable"
    );
}

#[tokio::test]
async fn replacing_a_full_host_releases_capacity_before_recovery_callbacks() {
    let host = Host::default();
    let (_old, _incoming) = host.connect();
    for id in 0..REQUEST_CAPACITY {
        host.notify("peerNotificationsChanged", json!({"revision":id}))
            .unwrap();
    }
    assert_eq!(host.capacity.available_permits(), 0);
    let (generation, mut incoming) = host.connect();
    host.notify(
        "peerNotificationsChanged",
        json!({"revision":32,"groups":[]}),
    )
    .unwrap();
    let request = incoming.recv().await.unwrap();
    assert_eq!(request["params"]["revision"], 32);
    host.reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    assert_eq!(host.capacity.available_permits(), REQUEST_CAPACITY);
}

#[tokio::test]
async fn web_activity_coalesces_while_the_host_is_processing_it() {
    let host = Host::default();
    let (generation, mut requests) = host.connect();
    host.notify("systemWebActivity", json!({})).unwrap();
    host.notify("systemWebActivity", json!({})).unwrap();
    let request = requests.recv().await.unwrap();
    assert!(requests.try_recv().is_err());
    host.reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    tokio::task::yield_now().await;
    host.notify("systemWebActivity", json!({})).unwrap();
    assert_eq!(
        requests.recv().await.unwrap()["method"],
        "systemWebActivity"
    );
}

#[tokio::test]
async fn generation_bound_calls_never_enter_a_replacement_host_queue() {
    let host = Arc::new(Host::default());
    let (old, _) = host.connect();
    let (current, mut incoming) = host.connect();
    assert_eq!(
        host.call_current(old, "bleScanRead", json!({}))
            .await
            .unwrap_err(),
        "host connection replaced"
    );
    assert!(incoming.try_recv().is_err());
    assert_eq!(host.state.lock().unwrap().next_id, 0);
    assert!(host.state.lock().unwrap().pending.is_empty());
    let active = host.clone();
    let call = tokio::spawn(async move {
        active
            .call_current(current, "bleScanRead", json!({"scan":"owned"}))
            .await
    });
    let request = incoming.recv().await.unwrap();
    assert_eq!(request["params"], json!({"scan":"owned"}));
    host.reply(current, json!({"id":request["id"],"result":true}))
        .unwrap();
    assert_eq!(call.await.unwrap().unwrap(), json!(true));
    host.disconnect(current);
    assert_eq!(
        host.call_current(current, "bleScanRead", json!({}))
            .await
            .unwrap_err(),
        "host connection replaced"
    );
    assert_eq!(host.state.lock().unwrap().next_id, 1);
}
#[tokio::test]
async fn replies_are_correlated_and_replacement_cannot_complete_new_calls() {
    let host = Arc::new(Host::default());
    let (generation, mut incoming) = host.connect();
    let call = {
        let host = host.clone();
        tokio::spawn(async move { host.call("first", json!({})).await })
    };
    let request = incoming.recv().await.unwrap();
    host.reply(generation, json!({"id":request["id"],"result":123}))
        .unwrap();
    assert_eq!(call.await.unwrap().unwrap(), json!(123));
    let call = {
        let host = host.clone();
        tokio::spawn(async move { host.call("second", json!({})).await })
    };
    let request = incoming.recv().await.unwrap();
    let (next, _) = host.connect();
    assert!(call.await.unwrap().unwrap_err().contains("replaced"));
    host.disconnect(generation);
    assert_eq!(host.state.lock().unwrap().generation, next);
    host.reply(generation, json!({"id":request["id"],"result":0}))
        .unwrap();
    assert!(host.state.lock().unwrap().sender.is_some());
    host.disconnect(next);
    assert!(host.state.lock().unwrap().sender.is_none());
}
#[tokio::test]
async fn cancel_and_disconnect_release_pending_requests() {
    let host = Arc::new(Host::default());
    let (generation, mut incoming) = host.connect();
    let call = {
        let host = host.clone();
        tokio::spawn(async move { host.call("cancel", json!({})).await })
    };
    incoming.recv().await.unwrap();
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled());
    assert!(host.state.lock().unwrap().pending.is_empty());
    let call = {
        let host = host.clone();
        tokio::spawn(async move { host.call("disconnect", json!({})).await })
    };
    incoming.recv().await.unwrap();
    host.disconnect(generation);
    assert!(call.await.unwrap().unwrap_err().contains("disconnected"));
}
#[tokio::test]
async fn capacity_is_bounded_and_errors_are_not_empty_successes() {
    let host = Arc::new(Host::default());
    let (generation, mut incoming) = host.connect();
    let mut workers = vec![];
    for _ in 0..32 {
        let host = host.clone();
        workers.push(tokio::spawn(async move {
            host.call("pending", json!({})).await
        }));
        incoming.recv().await.unwrap();
    }
    assert!(
        host.call("overflow", json!({}))
            .await
            .unwrap_err()
            .contains("capacity")
    );
    host.disconnect(generation);
    for worker in workers {
        assert!(worker.await.unwrap().is_err());
    }
    assert!(host.state.lock().unwrap().pending.is_empty());
}

#[tokio::test]
async fn startup_properties_need_no_host_and_keep_dynamic_samples_fresh() {
    let host = Arc::new(
        Host::with_startup_config(StartupConfig {
            app_version: "1.2.3".into(),
            model: "device".into(),
            ..plain_rs::platform::startup::config()
        })
        .unwrap(),
    );
    assert_eq!(host.startup().app_version, "1.2.3");
    assert_eq!(host.startup().model, "device");
    let (generation, mut incoming) = host.connect();
    for active in [true, false] {
        let caller = host.clone();
        let request = tokio::spawn(async move {
            caller
                .call("systemProperties", json!({"names":["wifiAwareRunning"]}))
                .await
                .unwrap()
        });
        let sample = incoming.recv().await.unwrap();
        assert_eq!(sample["params"]["names"], json!(["wifiAwareRunning"]));
        host.reply(
            generation,
            json!({"id":sample["id"],"result":{"wifiAwareRunning":active}}),
        )
        .unwrap();
        assert_eq!(request.await.unwrap(), json!({"wifiAwareRunning":active}));
    }
    assert!(incoming.try_recv().is_err());
}
#[test]
fn dynamic_properties_cannot_be_frozen_as_startup_information() {
    for key in [
        "wifiAwareRunning",
        "display.width",
        "batteryLevel",
        "android.battery",
        "permissions",
    ] {
        let mut value = startup_json();
        value[key] = Value::Null;
        assert!(
            serde_json::from_value::<StartupConfig>(value).is_err(),
            "{key}"
        );
    }
}

#[test]
fn startup_values_are_validated_before_the_host_is_created() {
    for (key, value) in [
        ("debug", json!(null)),
        ("model", json!(123)),
        ("wifiAwareSupported", json!("true")),
        ("recentFilesCatalog", json!(1)),
        ("systemMediaCatalog", json!("true")),
        ("socModel", json!(false)),
        ("deviceType", json!("UNKNOWN")),
        ("buildChannel", json!("unknown")),
        ("androidSdkVersion", json!(-1)),
        ("androidSdkVersion", json!(2147483648_i64)),
        ("androidBuildTime", json!("today")),
        ("supportedAbis", json!("arm64-v8a")),
        ("supportedAbis", json!(["arm64-v8a", 1])),
        ("supportedAbis", json!([""])),
        ("updateSource", json!("https://example.test/releases")),
        ("updateSource", json!({"url":123})),
    ] {
        let mut config = startup_json();
        config[key] = value;
        let result = serde_json::from_value::<StartupConfig>(config)
            .map_err(|error| error.to_string())
            .and_then(Host::with_startup_config);
        assert!(result.is_err(), "{key}");
    }
    let mut value = startup_json();
    value["socModel"] = Value::Null;
    let config = serde_json::from_value::<StartupConfig>(value).unwrap();
    assert!(Host::with_startup_config(config).is_ok());
}

#[test]
fn startup_accepts_sdk_abi_arrays_and_optional_update_sources() {
    for properties in [
        json!({"supportedAbis":["arm64-v8a","armeabi-v7a"],"updateSource":{"url":"https://example.test/releases","pageUrl":"https://example.test","assets":[{"nameSuffix":".apk"}]}}),
        json!({"supportedAbis":[],"updateSource":null}),
    ] {
        let mut value = startup_json();
        value["supportedAbis"] = properties["supportedAbis"].clone();
        value["updateSource"] = properties["updateSource"].clone();
        let config = serde_json::from_value::<StartupConfig>(value).unwrap();
        let host = Host::with_startup_config(config).unwrap();
        assert_eq!(
            json!(host.startup().supported_abis),
            properties["supportedAbis"]
        );
        assert_eq!(
            host.startup().update_source.is_some(),
            !properties["updateSource"].is_null()
        );
    }
}

#[tokio::test]
async fn absent_fixed_values_fail_without_a_host_request() {
    let host = Host::default();
    let (_generation, mut incoming) = host.connect();
    let error = host
        .call(
            "systemProperties",
            json!({"names":["model","wifiAwareRunning"]}),
        )
        .await
        .unwrap_err();
    assert_eq!(error, "Unsupported dynamic system property: model");
    assert!(incoming.try_recv().is_err());
}

#[tokio::test]
async fn unknown_properties_never_fall_back_to_dynamic_host_requests() {
    let host = Host::default();
    let (_generation, mut incoming) = host.connect();
    for name in ["dataDir", "systemDbPath", "ipv4Addresses", "unknown"] {
        let error = host
            .call("systemProperties", json!({"names":["language", name]}))
            .await
            .unwrap_err();
        assert_eq!(
            error,
            format!("Unsupported dynamic system property: {name}")
        );
    }
    assert!(incoming.try_recv().is_err());
}

#[tokio::test]
async fn repeated_dynamic_names_are_sampled_once_per_request() {
    let host = Arc::new(Host::default());
    let (generation, mut incoming) = host.connect();
    let caller = host.clone();
    let call = tokio::spawn(async move {
        caller
            .call(
                "systemProperties",
                json!({"names":["language", "display.width", "language", "display.width"]}),
            )
            .await
    });
    let request = incoming.recv().await.unwrap();
    assert_eq!(
        request["params"]["names"],
        json!(["language", "display.width"])
    );
    host.reply(
        generation,
        json!({"id":request["id"],"result":{"language":"en","display.width":1080}}),
    )
    .unwrap();
    assert_eq!(
        call.await.unwrap().unwrap(),
        json!({"language":"en","display.width":1080})
    );
    assert!(incoming.try_recv().is_err());
}

#[tokio::test]
async fn cache_root_is_reused_from_startup_without_a_host_request() {
    let root = std::env::temp_dir().join("plain-cache-startup");
    let host = Host::with_startup_config(StartupConfig {
        cache_dir: Some(root.to_str().unwrap().into()),
        ..plain_rs::platform::startup::config()
    })
    .unwrap();
    let (_generation, mut incoming) = host.connect();
    for _ in 0..3 {
        assert_eq!(host.startup().cache_dir.as_deref(), root.to_str());
    }
    assert!(incoming.try_recv().is_err());
    assert!(
        Host::with_startup_config(StartupConfig {
            cache_dir: Some("relative".into()),
            ..plain_rs::platform::startup::config()
        })
        .is_err()
    );
}

#[tokio::test]
async fn invalidations_coalesce_while_host_capacity_is_full() {
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    for _ in 0..REQUEST_CAPACITY {
        host.notify("fixture", json!({})).unwrap();
    }
    host.invalidate("clipboardHistoryRefresh");
    host.invalidate("clipboardHistoryRefresh");
    let first = requests.recv().await.unwrap();
    host.reply(generation, json!({"id":first["id"],"result":null}))
        .unwrap();
    for _ in 1..REQUEST_CAPACITY {
        let request = requests.recv().await.unwrap();
        host.reply(generation, json!({"id":request["id"],"result":null}))
            .unwrap();
    }
    let refresh = tokio::time::timeout(Duration::from_secs(3), requests.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(refresh["method"], "clipboardHistoryRefresh");
    host.reply(generation, json!({"id":refresh["id"],"result":null}))
        .unwrap();
    assert!(requests.try_recv().is_err());
}
