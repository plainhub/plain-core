use super::*;
use std::sync::Arc;

#[tokio::test]
async fn support_uses_one_capability_sample_without_permission_or_method_probes() {
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let worker = {
        let host = host.clone();
        tokio::spawn(async move {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "systemCapabilities");
            host.reply(
                generation,
                serde_json::json!({"id":request["id"],"result":["MEDIA_DELETE","NOTES"]}),
            )
            .unwrap();
            assert!(requests.try_recv().is_err());
        })
    };
    let capabilities = snapshot(&host).await.unwrap();
    assert!(require_list(&capabilities, "MEDIA_DELETE").is_ok());
    assert!(require_list(&capabilities, "MEDIA_MOVE").is_err());
    worker.await.unwrap();
}

#[tokio::test]
async fn malformed_capabilities_are_errors_and_never_trigger_a_fallback() {
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let worker = {
        let host = host.clone();
        tokio::spawn(async move {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "systemCapabilities");
            host.reply(
                generation,
                serde_json::json!({"id":request["id"],"result":{"capabilities":[]}}),
            )
            .unwrap();
            assert!(requests.try_recv().is_err());
        })
    };
    assert!(snapshot(&host).await.is_err());
    worker.await.unwrap();
}
