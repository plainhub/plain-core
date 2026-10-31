#[tokio::test]
async fn main_device_fields_keep_alias_fragment_and_nested_type_behavior_without_host_calls() {
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let directory = tempfile::tempdir().unwrap();
    let db = Arc::new(plain_rs::db::Db::open(&directory.path().join("plain.db")).unwrap());
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let (events, _) = tokio::sync::broadcast::channel(16);
    let schema =
        crate::test_support::schema::build(host, events, prefs, db, directory.path().to_path_buf());
    let response = schema.execute("query { first: deviceInfo { ...Identity } second: deviceInfo { platform } } fragment Identity on DeviceInfo { platform display { __typename } android { __typename } }").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["first"]["platform"], data["second"]["platform"]);
    if cfg!(any(target_os = "android", target_os = "ios")) {
        assert_eq!(data["first"]["display"]["__typename"], "DisplayInfo");
    } else {
        assert!(data["first"]["display"].is_null());
    }
    if cfg!(target_os = "android") {
        assert_eq!(data["first"]["android"]["__typename"], "AndroidExtras");
    } else {
        assert!(data["first"]["android"].is_null());
    }
    assert!(requests.try_recv().is_err());
}
use plain_rs::host::bridge::Host;
use std::sync::Arc;
