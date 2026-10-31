use super::*;
use crate::server::ContentServer;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn native_main_schema_reads_and_stops_idle_mdns_without_initializing_the_host() {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    prefs.set("client_id", "mdns-fixture").unwrap();
    let server = ContentServer::start(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let native = server.native_graphql().unwrap();
    let response = native.execute(async_graphql::Request::new("{ mdnsState { runtimeId revision receiver running scanning capturing debugSessionCount services { instanceFqdn } packetsIn { time } packetsOut { time } } }")).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let state = &data["mdnsState"];
    assert!(!state["runtimeId"].as_str().unwrap().is_empty());
    assert_eq!(state["revision"], 0);
    assert_eq!(state["receiver"], false);
    assert_eq!(state["services"], json!([]));
    assert_eq!(state["packetsIn"], json!([]));
    let response = native
        .execute(async_graphql::Request::new("mutation { stopDiscovery }"))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["stopDiscovery"], true);
    let response = native
        .execute(async_graphql::Request::new(
            "{ mdnsState { runtimeId receiver } }",
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let stopped = response.data.into_json().unwrap();
    assert_eq!(stopped["mdnsState"]["runtimeId"], state["runtimeId"]);
    assert_eq!(stopped["mdnsState"]["receiver"], false);
    let response = native
        .execute(async_graphql::Request::new(
            "{ chatRecords(query: \"peer:unknown-peer\", offset: 0, limit: 20) { id } }",
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["chatRecords"], json!([]));
    server.shutdown().await;
}

#[test]
fn mdns_snapshot_projection_keeps_packet_timestamps_and_ipv6_without_loss() {
    let state: MdnsState = serde_json::from_value(json!({
        "runtimeId":"fixture", "revision":5000000000_i64, "receiver":true,"running":true,"scanning":false,"published":true,"manualScanning":false,"debugSessionCount":1,"capturing":true,
        "services":[{"serviceType":"_plain._tcp.local.","instanceName":"fixture","instanceFqdn":"fixture._plain._tcp.local.","hostname":"fixture.local.","port":8443,"txtRecords":["id=fixture"],"ips":[],"ipv6":["fe80::1%4"],"complete":true}],
        "packetsIn":[{"time":1790812800123_i64,"direction":"IN","srcIp":"::1","srcPort":5353,"dstIp":"ff02::fb","dstPort":5353,"size":100,"summary":"PTR","detail":"fixture"}],"packetsOut":[]
    })).unwrap();
    assert_eq!(state.revision, Long(5000000000));
    assert_eq!(state.packets_in[0].time, Long(1790812800123));
    assert_eq!(state.services[0].ipv6, ["fe80::1%4"]);
}

#[tokio::test]
async fn native_schema_does_not_expose_platform_scan_lifecycle() {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let server = ContentServer::start(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let native = server.native_graphql().unwrap();
    let retired = native
        .execute(async_graphql::Request::new(
            "{ mdnsDebug { services { hostname } } }",
        ))
        .await;
    assert!(!retired.errors.is_empty());
    for request in [
        "{ discoveryReply { id } }",
        "{ discoveryBleAdvertisement }",
        "mutation { beginBleScan }",
        "mutation { browseMdns { revision } }",
        "mutation { endBleScan(scanId: \"retired\") }",
    ] {
        let response = native.execute(async_graphql::Request::new(request)).await;
        assert!(!response.errors.is_empty());
    }
    server.shutdown().await;
}

#[tokio::test]
async fn main_schema_hostname_and_debug_reads_reuse_owned_idle_state() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let executor = core.native_graphql().unwrap();
    let request = async_graphql::Request::new(
        r#"mutation{setPref(key:"mdns_hostname",value:" renderer.local ")}"#,
    );
    let response = executor.execute(request).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        state
            .prefs
            .get::<String>(plain_rs::prefs::pref_keys::MDNS_HOSTNAME)
            .unwrap()
            .unwrap(),
        "renderer.local"
    );
    let response = executor
        .execute(async_graphql::Request::new(
            "{mdnsState{receiver services{hostname} activity{time event detail}}}",
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let value = response.data.into_json().unwrap();
    assert_eq!(value["mdnsState"]["receiver"], false);
    assert_eq!(value["mdnsState"]["services"], json!([]));
    assert_eq!(value["mdnsState"]["activity"], json!([]));
    let invalid = executor
        .execute(async_graphql::Request::new(
            r#"mutation{setPref(key:"mdns_hostname",value:"invalid.example")}"#,
        ))
        .await;
    assert!(!invalid.errors.is_empty());
    assert_eq!(
        plain_rs::prefs::security::snapshot(&state.prefs)
            .unwrap()
            .mdns_hostname,
        "renderer.local"
    );
    let remote = crate::graphql::executor::execute_graphql(
        &state.public,
        async_graphql::Request::new("{mdnsState{activity{event}}}"),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!remote.errors.is_empty());
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
    for document in [
        "{mdnsState{receiver}}",
        "{mdnsState{services{hostname} activity{event}}}",
        r#"mutation{acquireMdnsDebugSession(token:"00000000-0000-0000-0000-000000000001"){revision}}"#,
        "{systemSettings{mdnsHostname}}",
    ] {
        assert!(
            !executor
                .execute(async_graphql::Request::new(document))
                .await
                .errors
                .is_empty()
        );
    }
}

#[tokio::test]
async fn lightweight_browser_query_does_not_collect_full_packet_capture_snapshots() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let executor = core.native_graphql().unwrap();
    for document in [
        "{mdnsState{services{hostname} activity{event}}}",
        "query { state:mdnsState { ...Snapshot } } fragment Snapshot on MdnsState { services{hostname} activity{event} }",
        "{mdnsState{activity{event}}}",
    ] {
        let response = executor
            .execute(async_graphql::Request::new(document))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
    }
    assert_eq!(
        state
            .mdns
            .full_snapshot_count
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    let response = executor
        .execute(async_graphql::Request::new(
            "{mdnsState{packetsIn{summary}}}",
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        state
            .mdns
            .full_snapshot_count
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    core.stop_core().await;
}
