use anyhow::{Result, ensure};
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::peers::management::Identity;
use plain_rs::services::peers::management::current_peer;
use serde_json::json;
use std::time::Duration;
use tokio::sync::watch;

pub async fn receive(
    state: &CoreRuntime,
    peer_id: &str,
    identity: &Identity,
    cancelled: &watch::Receiver<bool>,
) -> Result<()> {
    if !plain_rs::clipboard::sync::receives(&state.prefs, peer_id)?
        || !current_peer(state, peer_id, identity, cancelled).await?
    {
        return Ok(());
    }
    let Some(host_generation) = state.host.generation() else {
        return Ok(());
    };
    let capabilities = plain_rs::services::capabilities::snapshot(&state.host).await?;
    if !capabilities
        .iter()
        .any(|capability| capability == "CLIPBOARD")
    {
        return Ok(());
    }
    let base = identity.http_url()?;
    let data = plain_rs::services::peers::management_http::query(
        state,
        &base,
        &state.native_client_id().map_err(anyhow::Error::msg)?,
        &plain_rs::utils::base64::base64_decode(&identity.token),
        json!({"query":r#"{ clipboardItems(query:"",offset:0,limit:1) { id text sensitive } }"#}),
        4 * 1024 * 1024,
    )
    .await?;
    let items: Vec<plain_rs::clipboard::sync::Item> =
        serde_json::from_value(data["clipboardItems"].clone())?;
    ensure!(items.len() <= 1, "Invalid clipboard snapshot");
    if let Some(item) = items.first() {
        if !state.host.is_current(host_generation)
            || !current_peer(state, peer_id, identity, cancelled).await?
            || !plain_rs::clipboard::sync::prepare(
                &state.prefs,
                state.store.get()?.as_ref(),
                peer_id,
                item,
            )?
        {
            return Ok(());
        }
        plain_rs::services::clipboard::record_history(
            &state.db,
            &state.host,
            &item.text,
            peer_id,
            false,
        )
        .map_err(anyhow::Error::msg)?;
        state.clipboard_changes.received(&item.text);
        let result = state
            .host
            .call_current(
                host_generation,
                "systemSetClipboard",
                json!({"text":item.text}),
            )
            .await
            .map_err(anyhow::Error::msg)?;
        ensure!(result.as_bool() == Some(true), "Clipboard write failed");
        plain_rs::clipboard::sync::applied(state.store.get()?.as_ref(), peer_id, item)?;
    }
    Ok(())
}

pub async fn send(
    state: &CoreRuntime,
    peer_id: &str,
    identity: &Identity,
    cancelled: &watch::Receiver<bool>,
    text: &str,
    access: &watch::Receiver<Option<bool>>,
) -> Result<bool> {
    plain_rs::clipboard::validate_text(text).map_err(anyhow::Error::msg)?;
    if !current_peer(state, peer_id, identity, cancelled).await?
        || *access.borrow() != Some(true)
        || !plain_rs::clipboard::sync::sends(&state.prefs, peer_id)?
        || !plain_rs::services::device::access::is_enabled(&state.prefs, &["CLIPBOARD"])
        || !state.clipboard_changes.can_send(text)
    {
        return Ok(false);
    }
    let data = plain_rs::services::peers::management_http::query(
        state,
        &identity.http_url()?,
        &state.native_client_id().map_err(anyhow::Error::msg)?,
        &plain_rs::utils::base64::base64_decode(&identity.token),
        json!({"query":"mutation($text:String!){setClipboard(text:$text)}","variables":{"text":text}}),
        65536,
    ).await?;
    ensure!(
        data["setClipboard"].as_bool() == Some(true),
        "Remote clipboard write failed"
    );
    current_peer(state, peer_id, identity, cancelled).await
}

pub async fn follow(
    state: &CoreRuntime,
    peer_id: &str,
    identity: &Identity,
    cancelled: &watch::Receiver<bool>,
    changed: &tokio::sync::Notify,
    mut access: watch::Receiver<Option<bool>>,
) {
    let mut captures = state.clipboard_changes.subscribe();
    let mut preferences = state.events.subscribe();
    let mut host_changes = state.host.connection_changes();
    let mut stop = state.stop.clone();
    let mut cancellation = cancelled.clone();
    let mut receiving = plain_rs::clipboard::sync::receives(&state.prefs, peer_id).unwrap_or(false);
    let mut pending: Option<std::sync::Arc<plain_rs::services::clipboard::Capture>> = None;
    let mut retry_at = tokio::time::Instant::now();
    let mut delay = Duration::from_secs(1);
    while !*cancellation.borrow() && !*stop.borrow() {
        if !plain_rs::clipboard::sync::sends(&state.prefs, peer_id).unwrap_or(false)
            || !plain_rs::services::device::access::is_enabled(&state.prefs, &["CLIPBOARD"])
            || *access.borrow() == Some(false)
        {
            pending = None;
        }
        let can_push = *access.borrow() == Some(true);
        let mut refresh = false;
        tokio::select! {
            biased;
            _ = stop.changed() => return,
            _ = cancellation.changed() => return,
            result = captures.changed() => {
                if result.is_err() { return; }
                pending = captures.borrow_and_update().clone().filter(|capture| capture.eligible);
                retry_at = tokio::time::Instant::now();
                delay = Duration::from_secs(1);
            },
            result = access.changed() => {
                if result.is_err() { return; }
                retry_at = tokio::time::Instant::now();
            },
            event = preferences.recv() => match event {
                Ok(event) if event.event_type == "PREFS_UPDATED" => {
                    let enabled = plain_rs::clipboard::sync::receives(&state.prefs, peer_id).unwrap_or(false);
                    refresh = enabled && !receiving;
                    receiving = enabled;
                },
                Ok(event) if event.event_type == "PERMISSIONS_UPDATED" => refresh = true,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => { pending = None; refresh = true; },
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                _ => {},
            },
            result = host_changes.changed() => {
                if result.is_err() { return; }
                pending = None;
                refresh = true;
            },
            _ = changed.notified() => refresh = true,
            _ = tokio::time::sleep_until(retry_at), if pending.is_some() && can_push => {
                let capture = pending.take().unwrap();
                if let Err(error) = send(state, peer_id, identity, cancelled, &capture.text, &access).await {
                    log::debug!("Peer clipboard send failed: {error}");
                    pending = Some(capture);
                    retry_at = tokio::time::Instant::now() + delay;
                    delay = plain_rs::services::peers::management::next_delay(delay);
                }
            },
        }
        if refresh {
            if let Err(error) = receive(state, peer_id, identity, cancelled).await {
                log::debug!("Peer clipboard receive failed: {error}");
            }
        }
    }
}
