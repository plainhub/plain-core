use super::*;
use crate::test_support::schema::SchemaFixture as PublicSchema;
use plain_rs::db::Db;
use plain_rs::prefs::Prefs;
use serde_json::{Value, json};
use std::sync::Arc;

fn stub(host: Arc<Host>, handler: impl Fn(&str, Value) -> Value + Send + 'static) {
    let (generation, mut requests) = host.connect();
    let host = host.clone();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let Some(id) = request["id"].as_u64() else {
                continue;
            };
            let result = handler(
                &request["method"].as_str().unwrap_or_default(),
                request["params"].clone(),
            );
            let _ = host.reply(generation, json!({ "id": id, "result": result }));
        }
    });
}

fn permission_sample(params: Value, grants: &[&str]) -> Value {
    let values: serde_json::Map<String, Value> = params["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            let name = value.as_str().unwrap();
            (name.to_owned(), Value::Bool(grants.contains(&name)))
        })
        .collect();
    json!({"granted":values})
}

fn fixture(
    handler: impl Fn(&str, Value) -> Value + Send + 'static,
) -> (tempfile::TempDir, PublicSchema) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    plain_rs::prefs::security::bootstrap(&prefs).unwrap();
    prefs.set("client_id", "c1").unwrap();
    prefs.set("url_token", "dG9rZW4=").unwrap();
    prefs.set("api_permissions", vec!["READ_SMS"]).unwrap();
    prefs.set("http_port", 8080).unwrap();
    prefs.set("https_port", 8443).unwrap();
    prefs.set("device_name", "Pixel 8").unwrap();
    prefs.set("developer_mode", true).unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let host = Arc::new(
        Host::with_startup_config(plain_rs::host::startup::StartupConfig {
            device_type: plain_rs::chat::enums::DeviceType::Phone,
            build_channel: "GITHUB".into(),
            downloads_dir: "/downloads".into(),
            debug: false,
            ..plain_rs::platform::startup::config()
        })
        .unwrap(),
    );
    stub(host.clone(), handler);
    let directory = dir.path().to_path_buf();
    let (events, _) = tokio::sync::broadcast::channel(16);
    let schema = crate::test_support::schema::build(host, events, prefs, db, directory);
    schema
        .data::<plain_rs::runtime::Runtime>()
        .unwrap()
        .logs
        .set_path(dir.path().join("plain.log"))
        .unwrap();
    (dir, schema)
}

fn properties() -> Value {
    json!({
        "name": "Test Device", "manufacturer": "Example", "model": "model", "osVersion": "14",
        "appVersion": "1.2.3", "appBuildNumber": "45", "language": "en", "cpuArch": "arm64-v8a",
        "socModel": null, "display.width": 1080, "display.height": 2400, "display.density": 2.75,
        "android.sdkVersion": 34, "android.buildBrand": "example", "android.buildTime": 1704067200000i64,
    })
}

#[tokio::test]
async fn device_info_carries_the_hardware_report_without_combined_host_facts() {
    let (_dir, schema) = fixture(|method, params| match method {
        "systemProperties" => {
            let values = properties();
            let mut selected = serde_json::Map::new();
            for name in params["names"].as_array().unwrap() {
                let name = name.as_str().unwrap();
                assert!(values.get(name).is_some(), "unexpected property {name}");
                selected.insert(name.into(), values[name].clone());
            }
            Value::Object(selected)
        }
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(
            r#"query { deviceInfo { name platform manufacturer model osVersion cpuArch
                        cpuModel totalMemory totalStorage
                        display { width height density }
                        android { sdkVersion buildBrand buildTime } } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let info = response.data.into_json().unwrap()["deviceInfo"].clone();
    assert!(info["totalMemory"].as_i64().unwrap() > 0);
    assert!(info["totalStorage"].as_i64().unwrap() > 0);
    assert!(!info["name"].as_str().unwrap().is_empty());
    if cfg!(any(target_os = "android", target_os = "ios")) {
        assert_eq!(info["name"], "Test Device");
        assert_eq!(info["display"]["density"], 2.75);
    } else {
        assert!(info["display"].is_null());
    }
    if cfg!(target_os = "android") {
        assert_eq!(info["platform"], "ANDROID");
        assert_eq!(info["android"]["sdkVersion"], 34);
        assert_eq!(info["android"]["buildBrand"], "example");
        assert_eq!(info["android"]["buildTime"], "2024-01-01T00:00:00.000Z");
    } else {
        assert!(info["android"].is_null());
    }
}

#[tokio::test]
async fn selecting_only_platform_and_nested_type_names_never_reads_sdk_properties() {
    let (_dir, schema) = fixture(|method, _| panic!("unexpected host call {method}"));
    let response = schema
        .execute("{ deviceInfo { platform display { __typename } android { __typename } } }")
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let info = response.data.into_json().unwrap()["deviceInfo"].clone();
    assert_eq!(
        info["display"].is_null(),
        !cfg!(any(target_os = "android", target_os = "ios"))
    );
    assert_eq!(info["android"].is_null(), !cfg!(target_os = "android"));
}

#[tokio::test]
async fn device_status_preserves_optional_battery_and_selected_values() {
    let (_dir, schema) = fixture(|method, _| match method {
        "systemBatterySample" => {
            json!({"level":null,"scale":null,"charging":false,"temperatureTenths":null})
        }
        "systemMemoryAvailable" => json!(0),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(
            r#"query { deviceStatus { uptimeSec batteryLevel charging cpuUsage
                        memoryAvailable storageAvailable temperatures { label celsius } } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let status = response.data.into_json().unwrap()["deviceStatus"].clone();
    assert!(status["uptimeSec"].as_i64().unwrap() >= 0);
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        if let Some(level) = status["batteryLevel"].as_i64() {
            assert!((0..=100).contains(&level));
        }
        assert!(status["charging"].is_boolean());
    } else {
        assert!(status["batteryLevel"].is_null());
        assert_eq!(status["charging"], false);
    }
    assert!((0.0..=100.0).contains(&status["cpuUsage"].as_f64().unwrap()));
    assert!(status["storageAvailable"].as_i64().unwrap() >= 0);
    if cfg!(target_os = "ios") {
        assert!(status["memoryAvailable"].is_null());
    } else if cfg!(target_os = "android") {
        assert_eq!(status["memoryAvailable"], 0);
    } else {
        assert!(status["memoryAvailable"].as_i64().unwrap() >= 0);
    }
    assert!(status["temperatures"].is_array());
}

#[tokio::test]
async fn the_app_root_exposes_the_identity_and_the_capability_lists() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let (_dir, schema) = fixture(move |method, params| {
        observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match method {
            "systemCapabilities" => json!(["SMS", "CONTACTS", "WAT"]),
            "systemPermissionFacts" => {
                permission_sample(params, &["RECORD_AUDIO", "READ_SMS", "ADB", "NOPE"])
            }
            other => panic!("unexpected host call {other}"),
        }
    });
    let response = schema
        .execute(
            r#"query { app { clientId urlToken httpPort httpsPort deviceName deviceType
                        buildChannel capabilities repeatedCapabilities:capabilities permissions repeatedPermissions:permissions developerMode debug } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    let app = response.data.into_json().unwrap()["app"].clone();
    assert_eq!(app["clientId"], "c1");
    assert_eq!(app["httpPort"], 8080);
    assert_eq!(app["deviceType"], "PHONE");
    assert_eq!(app["buildChannel"], "GITHUB");
    assert_eq!(app["developerMode"], true);
    assert!(
        app["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("SMS"))
    );
    assert!(
        !app["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("WAT"))
    );
    assert_eq!(app["repeatedCapabilities"], app["capabilities"]);
    assert_eq!(app["repeatedPermissions"], app["permissions"]);
    assert_eq!(
        app["permissions"],
        json!(["READ_SMS", "RECORD_AUDIO", "ADB"])
    );
}

/// A capability or permission this build has never heard of is dropped
/// rather than invented — a client cannot render a section it cannot
/// drive, and cannot request a permission it has no name for.
#[test]
fn unknown_capabilities_and_permissions_are_dropped() {
    assert_eq!(
        Capability::parse("PEER_DISCOVERY"),
        Some(Capability::PeerDiscovery)
    );
    assert_eq!(
        Capability::parse("BACKGROUND_SERVICE"),
        Some(Capability::BackgroundService)
    );
    assert_eq!(
        Capability::parse("FILE_BROWSER"),
        Some(Capability::FileBrowser)
    );
    assert_eq!(
        Capability::parse("BLE_DISCOVERY"),
        Some(Capability::BleDiscovery)
    );
    assert_eq!(Capability::parse("WIFI_AWARE"), Some(Capability::WifiAware));
    assert_eq!(Capability::parse("SMS"), Some(Capability::Sms));
    assert_eq!(Capability::parse("POMODORO"), Some(Capability::Pomodoro));
    assert_eq!(Capability::parse("TELEPORT"), None);
    assert_eq!(Permission::parse("READ_SMS"), Some(Permission::ReadSms));
    assert_eq!(Permission::parse("ADB"), Some(Permission::Adb));
    assert_eq!(Permission::parse("NOT_A_PERMISSION"), None);
}

#[tokio::test]
async fn a_log_search_filters_before_applying_the_page() {
    let (dir, schema) = fixture(|method, _| panic!("unexpected host call {method}"));
    std::fs::write(
        dir.path().join("plain.log"),
        "e boom 7\nnoise\ne boom 6\ne boom 5\nnoise\ne boom 4\ne boom 3\ne boom 2\ne boom 1\n",
    )
    .unwrap();
    let response = schema
        .execute(r#"query { appLogs(offset:5, limit:10, query:"text:boom size:>1") }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["appLogs"],
        json!(["e boom 6", "e boom 7"])
    );
}

#[tokio::test]
async fn a_log_query_without_text_pages_the_entire_file() {
    let (dir, schema) = fixture(|method, _| panic!("unexpected host call {method}"));
    std::fs::write(dir.path().join("plain.log"), "old\nnew\n").unwrap();
    let response = schema
        .execute(r#"query { appLogs(offset:0, limit:10, query:"size:>1") }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["appLogs"],
        json!(["new", "old"])
    );
}

#[tokio::test]
async fn reading_the_log_path_does_not_open_the_file_or_call_the_host() {
    let (dir, schema) = fixture(|method, _| panic!("unexpected host call {method}"));
    let response = schema.execute("{ appLogPath }").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["appLogPath"],
        dir.path().join("plain.log").to_string_lossy().as_ref()
    );
    assert!(!dir.path().join("plain.log").exists());
}

#[tokio::test]
async fn clearing_the_logs_truncates_the_configured_file() {
    let (dir, schema) = fixture(|method, _| panic!("unexpected host call {method}"));
    let path = dir.path().join("plain.log");
    std::fs::write(&path, "old\n").unwrap();
    let response = schema.execute("mutation { clearAppLogs }").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["clearAppLogs"], true);
    assert_eq!(std::fs::read(path).unwrap(), Vec::<u8>::new());
}

#[tokio::test]
async fn a_temp_value_is_echoed_rather_than_read_back() {
    let (_dir, schema) = fixture(|method, _| panic!("unexpected host call {method}"));
    let response = schema
        .execute(r#"mutation { setTempValue(key:"k", value:"v") { key value } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let pair = response.data.into_json().unwrap()["setTempValue"].clone();
    assert_eq!(pair["key"], "k");
    assert_eq!(pair["value"], "v");
}

#[tokio::test]
async fn discovery_commands_use_the_shared_runtime_and_platform_multicast_lease() {
    let _runtime = plain_rs::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("client_id", "discovery-test").unwrap();
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[4; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    stub(state.host.clone(), |method, _| match method {
        "systemCapabilities" => json!(["PEER_DISCOVERY"]),
        "mdnsMulticast" => json!(true),
        other => panic!("unexpected host call {other}"),
    });
    let execute = |document: &str| {
        state
            .public
            .execute(async_graphql::Request::new(document).data(state.clone()))
    };
    let response = execute("{isDiscovering}").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["isDiscovering"], false);
    let response = execute("mutation {startDiscovery}").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(state.mdns.snapshot()["scanning"], true);
    let response = execute("{isDiscovering}").await;
    assert_eq!(response.data.into_json().unwrap()["isDiscovering"], true);
    let response = execute("mutation {stopDiscovery}").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(state.mdns.snapshot()["scanning"], false);
    server.shutdown().await;
}

#[tokio::test]
async fn renaming_the_device_forwards_the_new_name() {
    let (_dir, schema) = fixture(|method, params| match method {
        "systemUpdateDeviceName" => {
            assert_eq!(params["name"], "New Name");
            json!(true)
        }
        "systemCapabilities" => json!(["APP_RELAUNCH"]),
        "systemRelaunchApp" => json!(true),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(r#"mutation { updateDeviceName(name:"New Name") relaunchApp }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["updateDeviceName"], true);
    assert_eq!(data["relaunchApp"], true);
}

#[tokio::test]
async fn local_app_permissions_use_os_grants_without_web_opt_in() {
    let (_dir, schema) = fixture(|method, params| match method {
        "systemPermissionFacts" => permission_sample(
            params,
            &["READ_SMS", "READ_CONTACTS", "RECORD_AUDIO", "UNKNOWN"],
        ),
        other => panic!("unexpected host call {other}"),
    });
    let query = "{ app { permissions } }";
    for (origin, expected) in [
        (
            crate::graphql::executor::RequestOrigin::Local,
            json!(["READ_SMS", "READ_CONTACTS", "RECORD_AUDIO"]),
        ),
        (
            crate::graphql::executor::RequestOrigin::Remote,
            json!(["READ_SMS", "RECORD_AUDIO"]),
        ),
    ] {
        let response = schema
            .execute(async_graphql::Request::new(query).data(origin))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(
            response.data.into_json().unwrap()["app"]["permissions"],
            expected
        );
    }
}

#[tokio::test]
async fn incomplete_permission_samples_fail_instead_of_granting_access() {
    let (_dir, schema) = fixture(|method, _| match method {
        "systemPermissionFacts" => json!({"granted":{"READ_SMS":true}}),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema.execute("{ app { permissions } }").await;
    assert_eq!(response.errors.len(), 1);
    assert!(
        response.errors[0]
            .message
            .starts_with("Missing permission sample:")
    );
}

#[tokio::test]
async fn app_preferences_do_not_call_the_platform() {
    let (_dir, schema) = fixture(|method, _| panic!("unexpected host call {method}"));
    let response = schema
        .execute("{app{clientId urlToken httpPort httpsPort deviceName developerMode appDir deviceType buildChannel downloadsDir debug}}")
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["app"]["urlToken"],
        "dG9rZW4="
    );
}

#[tokio::test]
async fn separate_app_queries_refresh_capabilities_and_permissions() {
    let capabilities = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let permissions = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let caps = capabilities.clone();
    let grants = permissions.clone();
    let (_directory, schema) = fixture(move |method, params| match method {
        "systemCapabilities" => {
            if caps.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                json!(["SMS"])
            } else {
                json!([])
            }
        }
        "systemPermissionFacts" => {
            if grants.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                permission_sample(params, &["READ_SMS"])
            } else {
                permission_sample(params, &[])
            }
        }
        other => panic!("unexpected host call {other}"),
    });
    let first = schema.execute("{ app { capabilities permissions } }").await;
    let second = schema.execute("{ app { capabilities permissions } }").await;
    assert!(first.errors.is_empty(), "{:?}", first.errors);
    assert!(second.errors.is_empty(), "{:?}", second.errors);
    let first = first.data.into_json().unwrap();
    let second = second.data.into_json().unwrap();
    assert!(
        first["app"]["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("SMS"))
    );
    assert!(
        !second["app"]["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("SMS"))
    );
    assert_eq!(first["app"]["permissions"], json!(["READ_SMS"]));
    assert_eq!(second["app"]["permissions"], json!([]));
    assert_eq!(capabilities.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(permissions.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[tokio::test]
async fn unsupported_relaunch_never_invokes_the_platform_action() {
    let (_directory, schema) = fixture(|method, _| match method {
        "systemCapabilities" => json!([]),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema.execute("mutation { relaunchApp }").await;
    assert_eq!(response.errors.len(), 1);
    assert!(
        response.errors[0]
            .message
            .contains("unsupported capability APP_RELAUNCH")
    );
}

#[tokio::test]
async fn main_app_capabilities_preserve_current_background_execution_opportunities() {
    for timer in [true, false] {
        let (_dir, schema) = fixture(move |method, _| {
            assert_eq!(method, "systemCapabilities");
            if timer {
                json!(["FEEDS", "BACKGROUND_WAKE", "BACKGROUND_TIMER"])
            } else {
                json!(["FEEDS", "BACKGROUND_WAKE"])
            }
        });
        let response = schema.execute("{ app { capabilities } }").await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        let names = data["app"]["capabilities"].as_array().unwrap();
        assert!(names.contains(&json!("FEEDS")));
        assert!(names.contains(&json!("BACKGROUND_WAKE")));
        assert_eq!(names.contains(&json!("BACKGROUND_TIMER")), timer);
    }
}
