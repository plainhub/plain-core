use crate::server::ServerState;
use plain_rs::events::{HOST_BINARY_TYPES, HOST_TEXT_TYPES, WsEvent};
use serde_json::{Value, json};
use tokio::sync::{Mutex, mpsc};

pub struct HostEvents {
    state: ServerState,
    generation: u64,
    preferences: Mutex<serde_json::Map<String, Value>>,
    requests: Mutex<mpsc::Receiver<Value>>,
}
struct ServingSession<'a>(&'a HostEvents);

impl Drop for ServingSession<'_> {
    fn drop(&mut self) {
        self.0.state.ble_advertising.close(self.0.generation);
        self.0.state.ble_scans.close_owner(self.0.generation);
        self.0.state.host.disconnect(self.0.generation);
    }
}

impl HostEvents {
    pub(crate) fn new(state: ServerState) -> Result<Self, String> {
        let preferences =
            crate::native::management::NativeManagement::new(state.clone()).preferences()?;
        let (generation, requests) = state.host.connect();
        let initial = (|| {
            let attempts = state
                .login_attempts
                .lock()
                .map_err(|_| "Login state unavailable")?;
            state
                .host
                .notify("systemWebLoginSnapshot", attempts.host_snapshot())?;
            drop(attempts);
            state
                .management
                .notifications
                .notify_snapshot(&state.host)?;
            state.management.notify_clipboard_snapshot(&state.host)?;
            state.subscription_sessions.notify_snapshot(&state.host)?;
            state.http_server.notify_snapshot(&state.host)?;
            state.host.notify("clipboardHistoryRefresh", json!({}))?;
            state.host.notify("dlnaRendererRefresh", json!({}))?;
            state
                .host
                .notify("sharedFolderDownloadsRefresh", json!({}))?;
            plain_rs::services::peers::peer_transport::notify_snapshot(&state)?;
            state.update_checks.download.notify_snapshot(&state.host)?;
            state.cast.notify_snapshot(&state.host)?;
            state.feeds.notify_snapshot()?;
            state.host.notify("audioPlaybackRefresh", json!({}))?;
            state.host.notify("audioLibraryRefresh", json!({}))?;
            state.host.notify("networkInterfacesChanged", json!({}))
        })();
        if let Err(error) = initial {
            state.host.disconnect(generation);
            return Err(error);
        }
        state.ble_scans.set_owner(generation);
        state.ble_advertising.connect(generation);
        plain_rs::services::settings::update_download::Runtime::schedule_reconcile(&state);
        let connected = state.core.clone();
        tokio::spawn(async move {
            tokio::join!(
                plain_rs::services::discovery::mdns_runtime::Runtime::host_connected(
                    connected.clone(),
                    generation
                ),
                plain_rs::services::casting::receiver::lifecycle::host_connected(
                    connected, generation
                ),
            );
        });
        Ok(Self {
            preferences: Mutex::new(preferences),
            state,
            generation,
            requests: Mutex::new(requests),
        })
    }

    pub async fn prefs_changed(&self) -> Result<(), String> {
        let mut previous = self.preferences.lock().await;
        let current =
            crate::native::management::NativeManagement::new(self.state.clone()).preferences()?;
        let changes: serde_json::Map<_, _> = current
            .iter()
            .filter(|(key, value)| previous.get(*key) != Some(*value))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        *previous = current;
        if changes.is_empty() {
            return Ok(());
        }
        tokio::select! {
            biased;
            _ = self.wait_disconnected() => Ok(()),
            result = self.state.host.call_current(
                self.generation, "prefsChanged", json!({"values": changes}),
            ) => result.map(|_| ()),
        }
    }

    pub fn capabilities(&self) -> Value {
        json!({"textTypes":HOST_TEXT_TYPES,"binaryTypes":HOST_BINARY_TYPES})
    }
    pub async fn wait_disconnected(&self) {
        let mut stop = self.state.stop.clone();
        let mut connection = self.state.host.connection_changes();
        if *stop.borrow() || !self.state.host.is_current(self.generation) {
            return;
        }
        tokio::select! {
            _ = stop.changed() => {},
            _ = connection.changed() => {},
        }
    }
    pub async fn next_request(&self) -> Option<Value> {
        let mut requests = self.requests.lock().await;
        tokio::select! {
            biased;
            _ = self.wait_disconnected() => None,
            value = requests.recv() => value,
        }
    }
    pub async fn serve<Requests, Pending>(
        self: std::sync::Arc<Self>,
        requests: Requests,
    ) -> Result<(), String>
    where
        Requests: Fn(String, Value) -> Pending,
        Pending: std::future::Future<Output = Result<Value, String>> + Send + 'static,
    {
        let _session = ServingSession(&self);
        self.serve_connected(requests).await
    }

    async fn serve_connected<Requests, Pending>(&self, requests: Requests) -> Result<(), String>
    where
        Requests: Fn(String, Value) -> Pending,
        Pending: std::future::Future<Output = Result<Value, String>> + Send + 'static,
    {
        let mut workers = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                biased;
                _ = self.wait_disconnected() => break,
                completed = workers.join_next(), if !workers.is_empty() => {
                    let (id, result) = completed
                        .ok_or("host worker unavailable")?
                        .map_err(|error| format!("host worker failed: {error}"))?;
                    let reply = match result {
                        Ok(result) => json!({"id": id, "result": result}),
                        Err(error) => json!({"id": id, "error": error}),
                    };
                    self.reply(reply)?;
                },
                request = self.next_request(), if workers.len() < plain_rs::host::bridge::REQUEST_CAPACITY => {
                    let Some(request) = request else { break };
                    let id = request.get("id").and_then(Value::as_u64)
                        .ok_or("invalid host request ID")?;
                    let method = request.get("method").and_then(Value::as_str)
                        .ok_or("invalid host request method")?.to_owned();
                    let params = request.get("params").cloned()
                        .ok_or("missing host request parameters")?;
                    if matches!(method.as_str(), "systemWebLoginRequested" | "systemWebLoginClosed" | "systemWebLoginCompleted" | "systemWebLoginSnapshot" | "peerNotificationsChanged" | "peerClipboardChanged" | "onlineClientsChanged" | "httpServerStatusChanged" | "clipboardHistoryRefresh" | "peerTransportsChanged" | "appUpdateDownloadChanged" | "dlnaSenderChanged" | "feedSyncChanged") {
                        let result = requests(method, params).await;
                        let reply = match result {
                            Ok(result) => json!({"id": id, "result": result}),
                            Err(error) => json!({"id": id, "error": error}),
                        };
                        self.reply(reply)?;
                    } else {
                        let pending = requests(method, params);
                        workers.spawn(async move { (id, pending.await) });
                    }
                },
            }
        }
        workers.abort_all();
        while workers.join_next().await.is_some() {}
        Ok(())
    }

    pub fn reply(&self, value: Value) -> Result<(), String> {
        self.state.host.reply(self.generation, value)
    }
    pub fn attach_resource(
        &self,
        id: &str,
    ) -> Result<plain_rs::host::resource_stream::NativeResourceStream, String> {
        self.state.resources.attach_native(id, self.generation)
    }

    pub fn submit_thumbnail(&self, token: &str, bytes: Vec<u8>) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        self.state.thumbnails.submit(token, bytes)
    }

    pub fn execution_state_changed(&self) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        self.state.feeds.execution_state_changed();
        Ok(())
    }

    pub fn grant_background_execution(&self, id: String, deadline_ms: i64) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        plain_rs::services::feeds::sync_service::schedule::grant(
            &self.state,
            id,
            deadline_ms,
            self.generation,
        )
        .map_err(|error| error.to_string())
    }

    pub async fn revoke_background_execution(&self, id: &str) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        plain_rs::services::feeds::sync_service::schedule::revoke(&self.state, id).await;
        Ok(())
    }

    pub async fn report_pairing_response(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        plain_rs::services::discovery::pairing_runtime::execute(
            &self.state,
            plain_rs::services::discovery::pairing_runtime::Request::Complete {
                response: serde_json::from_str(&payload).map_err(|error| error.to_string())?,
                sender_ip: String::new(),
            },
        )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
    }

    pub async fn report_notification(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }

        return crate::graphql::resolvers::notifications::public::publish(
            &self.state.prefs,
            &self.state.events,
            serde_json::from_str(&payload).map_err(|error| error.to_string())?,
        );
    }

    pub async fn report_image_catalog_changed(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }

        serde_json::from_str::<()>(&payload).map_err(|error| error.to_string())?;
        self.state.image_models.catalog_changed();
        return Ok(());
    }

    pub async fn report_network_interfaces_changed(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }

        serde_json::from_str::<()>(&payload).map_err(|error| error.to_string())?;
        self.state.mdns.network_changed(self.state.core.clone());
        self.state
            .network_revision
            .send_modify(|revision| *revision = revision.wrapping_add(1));
        self.state.host.invalidate("networkInterfacesChanged");
        return Ok(());
    }

    pub async fn report_ble_advertisement(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Advertisement {
            scan_id: String,
            short_id: String,
        }
        let value: Advertisement =
            serde_json::from_str(&payload).map_err(|error| error.to_string())?;
        plain_rs::services::discovery::ble_scan::seen(&self.state, &value.scan_id, &value.short_id)
            .await
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    pub async fn report_ble_scan_state(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct ScanState {
            scan_id: String,
            active: bool,
        }
        let value: ScanState = serde_json::from_str(&payload).map_err(|error| error.to_string())?;
        let id = uuid::Uuid::parse_str(&value.scan_id).map_err(|error| error.to_string())?;
        return self
            .state
            .ble_scans
            .report_state(self.generation, id, value.active)
            .map_err(|error| error.to_string());
    }

    pub async fn report_ble_advertising_facts(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }

        let facts = serde_json::from_str(&payload).map_err(|error| error.to_string())?;
        return self
            .state
            .ble_advertising
            .report(&self.state, self.generation, facts)
            .await
            .map_err(|error| error.to_string());
    }

    pub async fn report_screen_mirror_state(&self, payload: String) -> Result<(), String> {
        self.publish(WsEvent::host_text("SCREEN_MIRRORING", payload))
    }

    pub async fn report_screen_mirror_codec(&self, payload: String) -> Result<(), String> {
        self.publish(WsEvent::host_text("SCREEN_MIRROR_VIDEO_CODEC", payload))
    }

    pub async fn report_screen_mirror_audio_granted(&self, payload: String) -> Result<(), String> {
        self.publish(WsEvent::host_text("SCREEN_MIRROR_AUDIO_GRANTED", payload))
    }

    pub async fn report_sms_provider_changed(&self, payload: String) -> Result<(), String> {
        self.publish(WsEvent::host_text("SMS_PROVIDER_CHANGED", payload))
    }

    pub async fn report_sms_part_sent(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }

        self.state
            .sms_sends
            .report(serde_json::from_str(&payload).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    pub async fn report_clipboard(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }

        return plain_rs::services::clipboard::report(&self.state, &payload);
    }

    pub async fn report_permissions(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }

        self.state.mdns.permissions_changed(self.state.core.clone());
        if let Err(error) = self
            .state
            .ble_advertising
            .reconcile(&self.state, self.generation)
            .await
        {
            log::warn!("BLE advertising permission recovery failed: {error}");
        }

        self.publish(WsEvent::host_text("PERMISSIONS_UPDATED", payload))
    }

    pub async fn report_dlna_position(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        plain_rs::services::casting::receiver::reports::accept(
            &self.state,
            self.generation,
            "DLNA_POSITION_REPORTED",
            &payload,
        )
        .await
        .map(|_| ())
    }

    pub async fn report_dlna_playback(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        plain_rs::services::casting::receiver::reports::accept(
            &self.state,
            self.generation,
            "DLNA_PLAYBACK_REPORTED",
            &payload,
        )
        .await
        .map(|_| ())
    }

    pub async fn report_dlna_seek_consumed(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        plain_rs::services::casting::receiver::reports::accept(
            &self.state,
            self.generation,
            "DLNA_SEEK_CONSUMED_REPORTED",
            &payload,
        )
        .await
        .map(|_| ())
    }

    pub async fn report_dlna_stopped(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        plain_rs::services::casting::receiver::reports::accept(
            &self.state,
            self.generation,
            "DLNA_STOPPED_REPORTED",
            &payload,
        )
        .await
        .map(|_| ())
    }

    pub async fn report_audio_started(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        plain_rs::services::audio::reports::accept(
            &self.state,
            self.generation,
            "AUDIO_STARTED_REPORTED",
            &payload,
        )
        .await
        .map(|_| ())
    }

    pub async fn report_audio_completed(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        plain_rs::services::audio::reports::accept(
            &self.state,
            self.generation,
            "AUDIO_COMPLETED_REPORTED",
            &payload,
        )
        .await
        .map(|_| ())
    }

    pub async fn report_audio_progress(&self, payload: String) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        plain_rs::services::audio::reports::accept(
            &self.state,
            self.generation,
            "AUDIO_PROGRESS_REPORTED",
            &payload,
        )
        .await
        .map(|_| ())
    }

    pub fn report_screen_mirror_video(&self, payload: Vec<u8>) -> Result<(), String> {
        self.publish(WsEvent::host_binary("SCREEN_MIRROR_VIDEO", payload))
    }

    pub fn report_screen_mirror_audio(&self, payload: Vec<u8>) -> Result<(), String> {
        self.publish(WsEvent::host_binary("SCREEN_MIRROR_AUDIO", payload))
    }

    pub async fn publish_text(&self, kind: &str, payload: String) -> Result<(), String> {
        match kind {
            "PAIRING_RESPONSE_REPORTED" => self.report_pairing_response(payload).await,
            "NOTIFICATION_REPORTED" => self.report_notification(payload).await,
            "IMAGE_CATALOG_CHANGED" => self.report_image_catalog_changed(payload).await,
            "NETWORK_INTERFACES_CHANGED" => self.report_network_interfaces_changed(payload).await,
            "BLE_ADVERTISEMENT_REPORTED" => self.report_ble_advertisement(payload).await,
            "BLE_SCAN_STATE_REPORTED" => self.report_ble_scan_state(payload).await,
            "BLE_ADVERTISING_FACTS_REPORTED" => self.report_ble_advertising_facts(payload).await,
            "SCREEN_MIRRORING" => self.report_screen_mirror_state(payload).await,
            "SCREEN_MIRROR_VIDEO_CODEC" => self.report_screen_mirror_codec(payload).await,
            "SCREEN_MIRROR_AUDIO_GRANTED" => self.report_screen_mirror_audio_granted(payload).await,
            "SMS_PROVIDER_CHANGED" => self.report_sms_provider_changed(payload).await,
            "SMS_PART_SENT_REPORTED" => self.report_sms_part_sent(payload).await,
            "CLIPBOARD_REPORTED" => self.report_clipboard(payload).await,
            "PERMISSIONS_UPDATED" => self.report_permissions(payload).await,
            "DLNA_POSITION_REPORTED" => self.report_dlna_position(payload).await,
            "DLNA_PLAYBACK_REPORTED" => self.report_dlna_playback(payload).await,
            "DLNA_SEEK_CONSUMED_REPORTED" => self.report_dlna_seek_consumed(payload).await,
            "DLNA_STOPPED_REPORTED" => self.report_dlna_stopped(payload).await,
            "AUDIO_STARTED_REPORTED" => self.report_audio_started(payload).await,
            "AUDIO_COMPLETED_REPORTED" => self.report_audio_completed(payload).await,
            "AUDIO_PROGRESS_REPORTED" => self.report_audio_progress(payload).await,
            _ => {
                if !self.state.host.is_current(self.generation) {
                    return Err("host disconnected".into());
                }
                if plain_rs::services::discovery::pairing_runtime::accept_report(
                    &self.state,
                    kind,
                    &payload,
                )
                .await
                .map_err(|error| error.to_string())?
                {
                    return Ok(());
                }
                self.publish(WsEvent::host_text(kind, payload))
            }
        }
    }
    pub fn publish_binary(&self, kind: &str, payload: Vec<u8>) -> Result<(), String> {
        self.publish(WsEvent::host_binary(kind, payload))
    }
    fn publish(&self, event: Option<WsEvent>) -> Result<(), String> {
        if !self.state.host.is_current(self.generation) {
            return Err("host disconnected".into());
        }
        let event = event.ok_or("host event type not allowed")?;
        if crate::http::websocket::runtime::is_screen_event(event.event_type) {
            let _ = self.state.main_ws.screen_events.send(event);
        } else {
            let _ = self.state.events.send(event);
        }
        Ok(())
    }
}
impl Drop for HostEvents {
    fn drop(&mut self) {
        self.state.ble_advertising.close(self.generation);
        self.state.ble_scans.close_owner(self.generation);
        self.state.host.disconnect(self.generation);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/host/events.rs"]
mod tests;

#[cfg(test)]
pub(crate) struct TestHost(HostEvents);
#[cfg(test)]
impl TestHost {
    pub(crate) fn new(server: &crate::server::ContentServer) -> Self {
        Self(server.host_events().unwrap())
    }
    pub(crate) async fn next(
        &mut self,
    ) -> Option<Result<tokio_tungstenite::tungstenite::Message, String>> {
        loop {
            let value = self.0.next_request().await?;
            if crate::test_support::acknowledge_host_callback(
                &self.0.state.host,
                self.0.generation,
                &value,
            ) {
                continue;
            }
            return Some(Ok(tokio_tungstenite::tungstenite::Message::Text(
                value.to_string().into(),
            )));
        }
    }
    pub(crate) async fn send(
        &mut self,
        value: tokio_tungstenite::tungstenite::Message,
    ) -> Result<(), String> {
        self.0.reply(
            serde_json::from_str(value.to_text().map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?,
        )
    }
}

#[cfg(test)]
#[path = "../../tests/unit/host/events_runtime.rs"]
mod runtime_tests;
