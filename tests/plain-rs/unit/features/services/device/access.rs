use super::*;
use plain_libs::serde_json::Value;
#[test]
fn feature_grants_are_independent() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    prefs.set(API_PERMISSIONS, vec!["FILES", "IMAGES"]).unwrap();
    assert!(is_enabled(&prefs, &[AccessFeature::Files, AccessFeature::Images]));
    for feature in [AccessFeature::Audio, AccessFeature::Videos, AccessFeature::Docs, AccessFeature::Contacts, AccessFeature::Adb] {
        assert!(!is_enabled(&prefs, &[feature]));
    }
}
#[test]
fn platform_permission_names_do_not_grant_features() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    prefs
        .set(API_PERMISSIONS, vec!["WRITE_EXTERNAL_STORAGE", "READ_SMS"])
        .unwrap();
    assert!(!is_enabled(&prefs, &[AccessFeature::Files]));
    assert!(!is_enabled(&prefs, &[AccessFeature::Sms]));
}

#[tokio::test]
async fn base_media_access_is_sampled_without_capabilities_and_keeps_denials() {
    let host = std::sync::Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let responder = host.clone();
    let worker = tokio::spawn(async move {
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "systemAccessFacts");
        let names = json!(["IMAGES", "VIDEOS", "AUDIO", "DOCS", "FILES"]);
        assert_eq!(request["params"]["features"], names);
        let grants: plain_libs::serde_json::Map<String, Value> = names
            .as_array()
            .unwrap()
            .iter()
            .map(|name| (name.as_str().unwrap().to_owned(), json!(name == "IMAGES")))
            .collect();
        responder
            .reply(
                generation,
                json!({"id":request["id"], "result":{"granted":grants}}),
            )
            .unwrap();
    });
    assert_eq!(
        snapshot_with_capabilities(&host, &[]).await.unwrap(),
        vec![AccessFeature::Images]
    );
    worker.await.unwrap();
}
