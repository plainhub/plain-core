use super::*;
use std::sync::Arc;
use tokio::sync::mpsc;

fn fixture() -> (
    tempfile::TempDir,
    crate::assembly::server::ContentServer,
    ServerState,
    Arc<Runtime>,
    u64,
    mpsc::Receiver<serde_json::Value>,
) {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    prefs.set("client_id", "ble-advertising-fixture").unwrap();
    let core = crate::assembly::server::ContentServer::start_core(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[9; 32]),
        prefs,
    )
    .unwrap();
    let mut state = core.runtime_state();
    state.host = Arc::new(
        crate::host::bridge::Host::with_startup_config(crate::host::startup::StartupConfig {
            wifi_aware_supported: true,
            ..crate::platform::startup::config()
        })
        .unwrap(),
    );
    let (generation, requests) = state.host.connect();
    let runtime = Arc::new(Runtime::default());
    runtime.connect(generation);
    (directory, core, state, runtime, generation, requests)
}

async fn drive(
    state: &ServerState,
    generation: u64,
    requests: &mut mpsc::Receiver<serde_json::Value>,
    mut task: tokio::task::JoinHandle<Result<()>>,
    supported: bool,
    accepted: bool,
) -> (Result<()>, Vec<serde_json::Value>) {
    let mut calls = Vec::new();
    loop {
        tokio::select! {
            result = &mut task => return (result.unwrap(), calls),
            call = requests.recv() => {
                let call = call.unwrap();
                let result = match call["method"].as_str().unwrap() {
                    "systemCapabilities" => if supported { json!(["BLE_DISCOVERY"]) } else { json!([]) },
                    "bleAdvertising" => json!(accepted),
                    other => panic!("Unexpected host method: {other}"),
                };
                state.host.reply(generation, json!({"id":call["id"],"result":result})).unwrap();
                calls.push(call);
            }
        }
    }
}

fn report(
    state: &ServerState,
    runtime: &Arc<Runtime>,
    generation: u64,
    available: bool,
    aware_running: bool,
) -> tokio::task::JoinHandle<Result<()>> {
    let state = state.clone();
    let runtime = runtime.clone();
    tokio::spawn(async move {
        runtime
            .report(
                &state,
                generation,
                Facts {
                    lease: uuid::Uuid::from_u128(1),
                    available,
                    aware_running,
                },
            )
            .await
    })
}

fn reconcile(
    state: &ServerState,
    runtime: &Arc<Runtime>,
    generation: u64,
) -> tokio::task::JoinHandle<Result<()>> {
    let state = state.clone();
    let runtime = runtime.clone();
    tokio::spawn(async move { runtime.reconcile(&state, generation).await })
}

fn command(calls: &[serde_json::Value]) -> &serde_json::Value {
    let mut commands = calls
        .iter()
        .filter(|call| call["method"] == "bleAdvertising");
    let command = commands.next().unwrap();
    assert!(commands.next().is_none());
    &command["params"]
}

#[tokio::test]
async fn facts_drive_shared_bytes_and_skip_duplicate_platform_work() {
    let (_directory, core, state, runtime, generation, mut requests) = fixture();
    let (result, calls) = drive(
        &state,
        generation,
        &mut requests,
        report(&state, &runtime, generation, false, false),
        true,
        true,
    )
    .await;
    result.unwrap();
    assert!(calls.is_empty());
    let (result, calls) = drive(
        &state,
        generation,
        &mut requests,
        report(&state, &runtime, generation, true, false),
        true,
        true,
    )
    .await;
    result.unwrap();
    let first = command(&calls);
    assert_eq!(first["active"], true);
    assert_eq!(first["revision"], 1);
    assert_eq!(
        plain_rs::utils::base64::base64_decode(first["bytes"].as_str().unwrap()),
        plain_rs::discovery::advertisement::ble("ble-advertising-fixture", true, false).unwrap()
    );
    let (result, calls) = drive(
        &state,
        generation,
        &mut requests,
        report(&state, &runtime, generation, true, false),
        true,
        true,
    )
    .await;
    result.unwrap();
    assert!(calls.is_empty());
    let (result, calls) = drive(
        &state,
        generation,
        &mut requests,
        report(&state, &runtime, generation, true, true),
        true,
        true,
    )
    .await;
    result.unwrap();
    assert_eq!(command(&calls)["revision"], 2);
    assert_eq!(
        plain_rs::utils::base64::base64_decode(command(&calls)["bytes"].as_str().unwrap()),
        plain_rs::discovery::advertisement::ble("ble-advertising-fixture", true, true).unwrap()
    );
    let (result, calls) = drive(
        &state,
        generation,
        &mut requests,
        report(&state, &runtime, generation, false, true),
        true,
        true,
    )
    .await;
    result.unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        command(&calls),
        &json!({"active":false,"bytes":"","revision":3,"lease":uuid::Uuid::from_u128(1)})
    );
    let (result, calls) = drive(
        &state,
        generation,
        &mut requests,
        report(&state, &runtime, generation, false, true),
        true,
        true,
    )
    .await;
    result.unwrap();
    assert!(calls.is_empty());
    state.host.disconnect(generation);
    core.stop_core().await;
}

#[tokio::test]
async fn explicit_reconciliation_refreshes_capabilities_without_freezing_mutable_state() {
    let (_directory, core, state, runtime, generation, mut requests) = fixture();
    let (result, calls) = drive(
        &state,
        generation,
        &mut requests,
        report(&state, &runtime, generation, true, false),
        false,
        true,
    )
    .await;
    result.unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["method"], "systemCapabilities");
    let (result, calls) = drive(
        &state,
        generation,
        &mut requests,
        reconcile(&state, &runtime, generation),
        true,
        true,
    )
    .await;
    result.unwrap();
    assert_eq!(command(&calls)["active"], true);
    let (result, calls) = drive(
        &state,
        generation,
        &mut requests,
        reconcile(&state, &runtime, generation),
        false,
        true,
    )
    .await;
    result.unwrap();
    assert_eq!(command(&calls)["active"], false);
    assert_eq!(command(&calls)["revision"], 2);
    state.host.disconnect(generation);
    core.stop_core().await;
}

#[tokio::test]
async fn a_rejected_command_remains_retryable_for_the_same_facts() {
    let (_directory, core, state, runtime, generation, mut requests) = fixture();
    let (result, calls) = drive(
        &state,
        generation,
        &mut requests,
        report(&state, &runtime, generation, true, false),
        true,
        false,
    )
    .await;
    assert!(result.is_err());
    assert_eq!(command(&calls)["revision"], 1);
    let (result, calls) = drive(
        &state,
        generation,
        &mut requests,
        report(&state, &runtime, generation, true, false),
        true,
        true,
    )
    .await;
    result.unwrap();
    assert_eq!(command(&calls)["revision"], 2);
    state.host.disconnect(generation);
    core.stop_core().await;
}

#[tokio::test]
async fn host_replacement_rejects_pending_results_and_old_cleanup_cannot_clear_new_work() {
    let (_directory, core, state, runtime, generation, mut requests) = fixture();
    let old = report(&state, &runtime, generation, true, false);
    let capabilities = requests.recv().await.unwrap();
    state
        .host
        .reply(
            generation,
            json!({"id":capabilities["id"],"result":["BLE_DISCOVERY"]}),
        )
        .unwrap();
    let old_command = requests.recv().await.unwrap();
    assert_eq!(old_command["method"], "bleAdvertising");
    let (replacement, mut new_requests) = state.host.connect();
    runtime.connect(replacement);
    assert!(old.await.unwrap().is_err());
    runtime.close(generation);
    let (result, calls) = drive(
        &state,
        replacement,
        &mut new_requests,
        report(&state, &runtime, replacement, true, true),
        true,
        true,
    )
    .await;
    result.unwrap();
    assert_eq!(command(&calls)["revision"], 2);
    runtime.connect(generation);
    runtime.connect(replacement);
    assert_eq!(runtime.state.lock().unwrap().owner, Some(replacement));
    assert_eq!(
        runtime.state.lock().unwrap().facts,
        Some(Facts {
            lease: uuid::Uuid::from_u128(1),
            available: true,
            aware_running: true
        })
    );
    assert!(runtime.state.lock().unwrap().applied.is_some());
    assert!(
        runtime
            .report(
                &state,
                generation,
                Facts {
                    lease: uuid::Uuid::from_u128(1),
                    available: false,
                    aware_running: false
                }
            )
            .await
            .is_err()
    );
    assert!(new_requests.try_recv().is_err());
    runtime.close(replacement);
    runtime.connect(generation);
    runtime.connect(replacement);
    assert!(runtime.state.lock().unwrap().owner.is_none());
    assert!(runtime.state.lock().unwrap().facts.is_none());
    assert_eq!(runtime.state.lock().unwrap().latest_generation, replacement);
    state.host.disconnect(replacement);
    core.stop_core().await;
}

#[tokio::test]
async fn queued_unavailability_prevents_a_stale_start_after_capability_sampling() {
    let (_directory, core, state, runtime, generation, mut requests) = fixture();
    let old = report(&state, &runtime, generation, true, false);
    let capabilities = requests.recv().await.unwrap();
    let unavailable = runtime.report(
        &state,
        generation,
        Facts {
            lease: uuid::Uuid::from_u128(1),
            available: false,
            aware_running: false,
        },
    );
    tokio::pin!(unavailable);
    assert!(futures_util::poll!(unavailable.as_mut()).is_pending());
    state
        .host
        .reply(
            generation,
            json!({"id":capabilities["id"],"result":["BLE_DISCOVERY"]}),
        )
        .unwrap();
    old.await.unwrap().unwrap();
    unavailable.await.unwrap();
    assert!(requests.try_recv().is_err());
    state.host.disconnect(generation);
    core.stop_core().await;
}

#[test]
fn facts_accept_only_typed_platform_readings() {
    assert!(
        serde_json::from_value::<Facts>(
            json!({"available":true,"awareRunning":false,"lease":uuid::Uuid::from_u128(1)})
        )
        .is_ok()
    );
    for value in [
        json!({"available":"true","awareRunning":false}),
        json!({"available":true,"awareRunning":false,"active":true}),
        json!({"available":true}),
    ] {
        assert!(serde_json::from_value::<Facts>(value).is_err());
    }
}

#[tokio::test]
async fn a_new_sdk_lease_receives_a_command_even_when_the_bytes_are_unchanged() {
    let (_directory, core, state, runtime, generation, mut requests) = fixture();
    let (result, calls) = drive(
        &state,
        generation,
        &mut requests,
        report(&state, &runtime, generation, true, false),
        true,
        true,
    )
    .await;
    result.unwrap();
    let first = command(&calls).clone();
    let task_state = state.clone();
    let task_runtime = runtime.clone();
    let task = tokio::spawn(async move {
        task_runtime
            .report(
                &task_state,
                generation,
                Facts {
                    lease: uuid::Uuid::from_u128(2),
                    available: true,
                    aware_running: false,
                },
            )
            .await
    });
    let (result, calls) = drive(&state, generation, &mut requests, task, true, true).await;
    result.unwrap();
    assert_eq!(command(&calls)["bytes"], first["bytes"]);
    assert_eq!(command(&calls)["revision"], 2);
    assert_eq!(command(&calls)["lease"], json!(uuid::Uuid::from_u128(2)));
    state.host.disconnect(generation);
    core.stop_core().await;
}
