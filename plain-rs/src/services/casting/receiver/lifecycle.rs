use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::casting::dlna_sender_runtime::acquire_permission;
use plain_rs::services::casting::dlna_sender_runtime::release_permission;
use std::sync::atomic::Ordering;

fn active_port(state: &CoreRuntime) -> Option<u16> {
    if *state.stop.borrow() || !state.peer_status.public_active.load(Ordering::SeqCst) {
        return None;
    }
    u16::try_from(state.http_server.snapshot().http_port)
        .ok()
        .filter(|port| *port > 0)
}

pub async fn stop_locked(state: &CoreRuntime) {
    state.dlna.stop().await;
    state.dlna.set_retrying(false).await;
    release_permission(state, &state.cast.receiver_lease).await;
}

async fn start_locked(state: &CoreRuntime, port: u16) -> Result<(), String> {
    let result: Result<(), String> = async {
        acquire_permission(state, &state.cast.receiver_lease).await?;
        if active_port(state) != Some(port) {
            release_permission(state, &state.cast.receiver_lease).await;
            return Err("Public HTTP listener changed".into());
        }
        if state.dlna.is_running() && state.dlna.state.read().await.port != port {
            stop_locked(state).await;
            acquire_permission(state, &state.cast.receiver_lease).await?;
        }
        state.dlna.start(port).await;
        if !state.dlna.is_running() {
            release_permission(state, &state.cast.receiver_lease).await;
            return Err(state.dlna.state.read().await.start_error.clone());
        }
        Ok(())
    }
    .await;
    if let Err(error) = &result {
        state.dlna.set_start_error(error.clone()).await;
    }
    result
}

async fn reconcile_locked(state: &CoreRuntime) -> Result<(), String> {
    if *state.stop.borrow() {
        return Err("Core stopped".into());
    }
    if plain_rs::prefs::dlna::enabled(&state.prefs) {
        if let Some(port) = active_port(state) {
            return start_locked(state, port).await;
        }
    }
    let renderer = state.dlna.state.read().await;
    let needs_stop = renderer.is_running
        || !renderer.media_uri.is_empty()
        || renderer.pending_cast_request.is_some()
        || renderer.is_retrying
        || !renderer.start_error.is_empty();
    drop(renderer);
    let lease_held = state.cast.receiver_lease.lock().unwrap().is_some();
    if needs_stop || lease_held {
        stop_locked(state).await;
    }
    Ok(())
}

pub async fn reconcile(state: &CoreRuntime) -> Result<(), String> {
    let _operation = state.cast.receiver_operations.lock().await;
    reconcile_locked(state).await
}

pub async fn retry_locked(state: &CoreRuntime) -> Result<(), String> {
    if !plain_rs::prefs::dlna::enabled(&state.prefs) {
        return Err("DLNA receiver is disabled".into());
    }
    let port = active_port(state).ok_or("Public HTTP server is not running")?;
    stop_locked(state).await;
    state.dlna.set_retrying(true).await;
    let result = start_locked(state, port).await;
    state.dlna.set_retrying(false).await;
    result
}

pub async fn host_connected(state: CoreRuntime, generation: u64) {
    if !plain_rs::prefs::dlna::enabled(&state.prefs) {
        return;
    }
    let _operation = state.cast.receiver_operations.lock().await;
    if *state.stop.borrow() || !state.host.is_current(generation) {
        return;
    }
    if let Err(error) = reconcile_locked(&state).await {
        log::warn!("DLNA receiver host recovery: {error}");
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/services/casting/receiver/lifecycle.rs"]
mod tests;
