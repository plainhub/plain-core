use crate::server::ServerState;

fn changed(state: &ServerState, id: &str) {
    state.transport.forget(id);
    state.prewarmer.forget(id);
    if let Err(error) = state.peer_status.outgoing.reconnect(state) {
        log::warn!("Peer status reconnect after mutation: {error}");
    }
    plain_rs::services::peers::peer_transport::notify(state);
    plain_rs::services::peers::status_runtime::emit(state, id, false);
}

pub(crate) async fn remove(state: &ServerState, id: &str) -> anyhow::Result<bool> {
    let state = state.clone();
    let id = id.to_owned();
    let worker = state.clone();
    let changed = tokio::task::spawn_blocking(move || remove_record(&worker, &id)).await??;
    state.aware.refresh(&state).await?;
    Ok(changed)
}
fn remove_record(state: &ServerState, id: &str) -> anyhow::Result<bool> {
    let _commit = state.pairing_runtime.commit.lock().unwrap();
    state.pairing_runtime.attempts.cancel_peer(state, id);
    let removed = plain_rs::chat::attachments::app_file_store::chat_deletion::delete(
        &state.db,
        &state.directory,
        plain_rs::chat::attachments::app_file_store::chat_deletion::Selection::PeerRecord(id),
    )? != 0;
    if removed {
        changed(state, id);
        plain_rs::services::discovery::pairing_runtime::event(
            state,
            plain_rs::chat::events::WS_MESSAGE_DELETED,
            serde_json::json!(format!("peer:{id}")),
        );
    }
    Ok(removed)
}

pub(crate) async fn unpair(state: &ServerState, id: &str) -> anyhow::Result<bool> {
    let state = state.clone();
    let id = id.to_owned();
    let worker = state.clone();
    let changed = tokio::task::spawn_blocking(move || unpair_record(&worker, &id)).await??;
    state.aware.refresh(&state).await?;
    Ok(changed)
}
fn unpair_record(state: &ServerState, id: &str) -> anyhow::Result<bool> {
    let _commit = state.pairing_runtime.commit.lock().unwrap();
    state.pairing_runtime.attempts.cancel_peer(state, id);
    let changed_peer = plain_rs::db::chat_store::peers::unpair(&state.db, id)?;
    if changed_peer {
        changed(state, id);
    }
    Ok(changed_peer)
}
