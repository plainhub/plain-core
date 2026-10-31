use crate::server::ContentServer;
use serde_json::json;

#[tokio::test]
async fn receiver_preserves_state_rules_and_refuses_operations_after_stop() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let management = core.native_management().unwrap();
    assert_eq!(management.dlna_receiver().await.unwrap()["enabled"], false);
    let response = core
        .native_graphql()
        .unwrap()
        .execute("mutation{setPref(key:\"dlna\",value:true)}".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(management.dlna_receiver().await.unwrap()["enabled"], true);
    assert_eq!(
        management.dlna_receiver().await.unwrap()["rules"],
        json!({"allowed":[],"denied":[]})
    );
    let mut events = state.events.subscribe();
    management.accept_dlna_cast(false).await.unwrap();
    management.reject_dlna_cast(false).await.unwrap();
    assert!(events.try_recv().is_err());
    core.stop_core().await;
    assert!(management.dlna_receiver().await.is_err());
    assert!(management.retry_dlna_receiver().await.is_err());
}

async fn set_enabled(core: &ContentServer, enabled: bool) -> Result<bool, String> {
    core.native_management()
        .unwrap()
        .set_pref("dlna", json!(enabled))
        .await
}

async fn snapshot(core: &ContentServer) -> serde_json::Value {
    core.native_management()
        .unwrap()
        .dlna_receiver()
        .await
        .unwrap()
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn enabling_receiver_without_samba_uses_the_public_listeners_actual_bound_port() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let (http, _) = core
        .start_public(
            0,
            0,
            cert.cert.pem().into_bytes(),
            cert.key_pair.serialize_pem().into_bytes(),
        )
        .await
        .unwrap();
    assert_ne!(http, 0);
    let state = core.runtime_state();
    let host = state.host.clone();
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&host);
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            if request["method"] == "systemCapabilities" {
                host.reply(generation, json!({"id":request["id"],"result":[]}))
                    .unwrap();
                continue;
            }
            assert_eq!(request["method"], "mdnsMulticast");
            let acquire = request["params"]["acquire"].as_bool().unwrap();
            host.reply(generation, json!({"id":request["id"],"result":acquire}))
                .unwrap();
        }
    });
    assert!(set_enabled(&core, true).await.unwrap());
    let value = snapshot(&core).await;
    assert_eq!(value["enabled"], true);
    assert_eq!(value["isRunning"], true, "{value}");
    assert_eq!(value["port"], http);
    core.stop_public().await;
    assert_eq!(snapshot(&core).await["isRunning"], false);
    assert_eq!(snapshot(&core).await["enabled"], true);
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let (restarted_http, _) = core
        .start_public(
            0,
            0,
            cert.cert.pem().into_bytes(),
            cert.key_pair.serialize_pem().into_bytes(),
        )
        .await
        .unwrap();
    let restarted = snapshot(&core).await;
    assert_eq!(restarted["isRunning"], true);
    assert_eq!(restarted["port"], restarted_http);
    assert!(set_enabled(&core, false).await.unwrap());
    assert_eq!(snapshot(&core).await["isRunning"], false);
    core.shutdown().await;
}
