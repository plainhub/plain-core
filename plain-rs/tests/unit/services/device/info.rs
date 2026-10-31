use super::*;
use plain_rs::host::startup::StartupConfig;
use serde_json::json;
use std::sync::Arc;

fn android_startup_config() -> StartupConfig {
    StartupConfig {
        android_sdk_version: Some(35),
        android_version_code_name: Some("REL".into()),
        android_security_patch: Some("2024-01-01".into()),
        android_bootloader: Some("bootloader".into()),
        android_fingerprint: Some("fingerprint".into()),
        android_hardware: Some("hardware".into()),
        android_radio_version: Some("radio".into()),
        android_board: Some("board".into()),
        android_build_brand: Some("brand".into()),
        android_build_number: Some("build".into()),
        android_device: Some("device".into()),
        android_java_vm_version: Some("vm".into()),
        android_gl_es_version: Some("3.2".into()),
        android_build_time: Some(1700000000123),
        ..plain_rs::platform::startup::config()
    }
}

#[test]
fn cpu_model_parser_preserves_the_pre_migration_fallback() {
    assert_eq!(
        cpu_model("processor: 0\nmodel name: Example CPU\nprocessor: 1\nmodel name: Other CPU\n"),
        Some("Example CPU".into())
    );
    assert_eq!(cpu_model("CPU part: 0x805\n"), None);
    assert_eq!(cpu_model("model name: "), None);
}

#[tokio::test]
async fn selected_properties_use_one_exchange_and_refresh_mutable_values() {
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let worker = {
        let host = host.clone();
        tokio::spawn(async move {
            for (language, density) in [("en", 2.0), ("fr", 3.0)] {
                let request = requests.recv().await.unwrap();
                assert_eq!(request["method"], "systemProperties");
                assert_eq!(
                    request["params"],
                    json!({"names":["language","display.density"]})
                );
                host.reply(generation, json!({"id":request["id"],"result":{"language":language,"display.density":density}})).unwrap();
            }
            assert!(requests.try_recv().is_err());
        })
    };
    for (language, density) in [("en", 2.0), ("fr", 3.0)] {
        let values = read_properties(&host, vec!["language".into(), "display.density".into()])
            .await
            .unwrap();
        let info = apply_sdk(
            direct(&[], std::path::Path::new("/")).unwrap(),
            values,
            true,
        )
        .unwrap();
        assert_eq!(info.language, language);
        assert_eq!(info.display.unwrap().density, density);
    }
    worker.await.unwrap();
}

#[tokio::test]
async fn invalid_or_missing_primitive_values_are_errors() {
    for result in [json!({}), json!({"language":42}), json!([])] {
        let host = Arc::new(Host::default());
        let (generation, mut requests) = host.connect();
        let worker = {
            let host = host.clone();
            tokio::spawn(async move {
                let request = requests.recv().await.unwrap();
                host.reply(generation, json!({"id":request["id"],"result":result}))
                    .unwrap();
            })
        };
        assert!(
            read_properties(&host, vec!["language".into()])
                .await
                .is_err()
        );
        worker.await.unwrap();
    }
}

#[tokio::test]
async fn direct_only_selection_never_requests_platform_properties() {
    let host = Host::default();
    assert!(read_properties(&host, Vec::new()).await.unwrap().is_empty());
    assert!(
        sdk_properties(&[
            "kernelVersion".into(),
            "totalStorage".into(),
            "totalMemory".into()
        ])
        .is_empty()
    );
    let info = collect(
        &host,
        std::path::PathBuf::from("/"),
        vec!["platform".into()],
        startup(&host).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(info.platform, platform());
}

#[test]
fn nested_type_selection_preserves_nullable_object_presence() {
    let host = Host::with_startup_config(android_startup_config()).unwrap();
    let info = apply_sdk(startup(&host).unwrap(), Map::new(), true).unwrap();
    assert!(info.display.is_some());
    assert!(info.android.is_some());
    let info = apply_sdk(
        direct(&[], std::path::Path::new("/")).unwrap(),
        Map::new(),
        false,
    )
    .unwrap();
    assert!(info.display.is_none());
    assert!(info.android.is_none());
}

#[test]
fn raw_build_milliseconds_and_soc_fallback_map_in_rust() {
    let expected_cpu = direct(&["cpuModel".into()], std::path::Path::new("/"))
        .unwrap()
        .cpu_model;
    let host = Host::with_startup_config(StartupConfig {
        soc_model: Some("unknown".into()),
        ..android_startup_config()
    })
    .unwrap();
    let info = startup(&host).unwrap();
    assert_eq!(info.cpu_model, expected_cpu);
    let android = info.android.unwrap();
    assert_eq!(android.sdk_version, 35);
    assert_eq!(android.build_time.0.timestamp_millis(), 1700000000123);
}

#[tokio::test]
async fn fixed_device_information_is_reused_and_storage_remains_live() {
    let directory = tempfile::tempdir().unwrap();
    let host = Host::default();
    let mut fixed = direct(&[], directory.path()).unwrap();
    fixed.app_version = "startup-version".into();
    fixed.total_memory = Long(123456);
    fixed.total_storage = Long(-1);
    let info = collect(
        &host,
        directory.path().to_path_buf(),
        vec![
            "appVersion".into(),
            "totalMemory".into(),
            "totalStorage".into(),
        ],
        fixed,
    )
    .await
    .unwrap();
    assert_eq!(info.app_version, "startup-version");
    assert_eq!(info.total_memory.0, 123456);
    assert!(info.total_storage.0 >= 0);
}

#[tokio::test]
async fn fixed_device_information_is_reused_without_platform_requests() {
    let host = Host::with_startup_config(StartupConfig {
        name: "startup-device".into(),
        manufacturer: "startup-maker".into(),
        model: "startup-model".into(),
        os_version: "startup-os".into(),
        app_version: "startup-version".into(),
        app_build_number: "42".into(),
        cpu_arch: "startup-arch".into(),
        soc_model: Some("startup-cpu".into()),
        ..android_startup_config()
    })
    .unwrap();
    let (generation, mut requests) = host.connect();
    let mut fixed = startup(&host).unwrap();
    fixed.total_memory = Long(123);
    let fields = FIELDS
        .iter()
        .filter(|field| !matches!(**field, "totalStorage" | "language"))
        .map(|field| (*field).to_owned())
        .chain(
            ANDROID_FIELDS
                .iter()
                .map(|field| format!("android.{field}")),
        )
        .chain(std::iter::once("android".to_owned()))
        .collect::<Vec<_>>();
    for _ in 0..2 {
        let query = collect(
            &host,
            std::path::PathBuf::from("/"),
            fields.clone(),
            fixed.clone(),
        );
        tokio::pin!(query);
        let info = tokio::select! {
            request = requests.recv() => panic!("Unexpected platform request: {request:?}"),
            result = &mut query => result.unwrap(),
        };
        assert_eq!(info.name, "startup-device");
        assert_eq!(info.manufacturer, "startup-maker");
        assert_eq!(info.model, "startup-model");
        assert_eq!(info.os_version, "startup-os");
        assert_eq!(info.app_version, "startup-version");
        assert_eq!(info.app_build_number, "42");
        assert_eq!(info.cpu_arch, "startup-arch");
        assert_eq!(info.cpu_model.as_deref(), Some("startup-cpu"));
        assert_eq!(info.total_memory.0, 123);
        let android = info.android.unwrap();
        assert_eq!(android.sdk_version, 35);
        assert_eq!(android.build_time.0.timestamp_millis(), 1700000000123);
    }
    assert!(requests.try_recv().is_err());
    host.disconnect(generation);
}

#[tokio::test]
async fn default_device_name_reuses_startup_metadata_without_a_host_request() {
    let host = Host::with_startup_config(StartupConfig {
        name: "startup-name".into(),
        ..plain_rs::platform::startup::config()
    })
    .unwrap();
    let (generation, mut requests) = host.connect();
    for _ in 0..2 {
        assert_eq!(host.startup().name, "startup-name");
    }
    assert!(requests.try_recv().is_err());
    host.disconnect(generation);
}

#[test]
fn fixed_information_is_ready_without_a_runtime_or_blocking_task() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let host = Host::default();
    let mut fixed = direct(&[], std::path::Path::new("/")).unwrap();
    fixed.name = "startup-name".into();
    fixed.app_version = "startup-version".into();
    let mut query = std::pin::pin!(collect(
        &host,
        std::path::PathBuf::from("/does-not-exist"),
        vec!["name".into(), "appVersion".into(), "totalMemory".into()],
        fixed,
    ));
    let Poll::Ready(result) = query.as_mut().poll(&mut Context::from_waker(Waker::noop())) else {
        panic!("Fixed information must not dispatch background work");
    };
    let info = result.unwrap();
    assert_eq!(info.name, "startup-name");
    assert_eq!(info.app_version, "startup-version");
}
