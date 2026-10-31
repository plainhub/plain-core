use crate::server::PublicServer;
use crate::server::ServerState;
use tokio::sync::watch;

pub(crate) fn monitor(
    state: ServerState,
    mut failure: watch::Receiver<Option<String>>,
    mut stop: watch::Receiver<bool>,
) {
    let mut core_stop = state.stop.clone();
    tokio::spawn(async move {
        let error = loop {
            if let Some(error) = failure.borrow().clone() {
                break error;
            }
            tokio::select! {
                biased;
                _ = core_stop.changed() => {
                    let mut guard = state.public_server.lock().await;
                    if guard.as_ref().is_some_and(|server| server.stop.subscribe().same_channel(&stop)) {
                        guard.take();
                    }
                    return;
                }
                _ = stop.changed() => return,
                changed = failure.changed() => {
                    if changed.is_err() { return; }
                }
            }
        };
        let _shutdown = state.shutdown.lock().await;
        let mut server = {
            let mut guard = state.public_server.lock().await;
            if !guard
                .as_ref()
                .is_some_and(|server| server.stop.subscribe().same_channel(&stop))
            {
                return;
            }
            guard.take()
        };
        stop_locked(&state, &mut server).await;
        state.http_server.failed(error.clone());
        state.publish_http_status();
    });
}

pub(crate) struct PublicDiscovery {
    task: Option<tokio::task::JoinHandle<()>>,
}

impl PublicDiscovery {
    pub(crate) fn start(state: ServerState, public_stop: watch::Receiver<bool>) -> Self {
        let task = tokio::spawn(async move {
            let core_stop = state.stop.clone();
            let start = async {
                plain_rs::services::discovery::mdns_runtime::control(
                    &state,
                    plain_rs::services::discovery::mdns_runtime::Request::Publish {},
                )
                .await?;
                if state.host.startup().wifi_aware_supported {
                    plain_rs::services::peers::status_outgoing::ensure_aware(&state).await?;
                }
                Ok(())
            };
            let finish = async {
                state
                    .mdns
                    .execute(
                        &state,
                        plain_rs::services::discovery::mdns_runtime::Request::Unpublish {},
                    )
                    .await
                    .map(|_| ())
            };
            public_discovery_lifetime(core_stop, public_stop, start, finish).await;
        });
        Self { task: Some(task) }
    }

    async fn finish(mut self) {
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

impl Drop for PublicDiscovery {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn public_discovery_lifetime(
    mut core_stop: watch::Receiver<bool>,
    mut public_stop: watch::Receiver<bool>,
    start: impl std::future::Future<Output = anyhow::Result<()>>,
    finish: impl std::future::Future<Output = anyhow::Result<()>>,
) {
    if *core_stop.borrow() || *public_stop.borrow() {
        return;
    }
    tokio::select! {
        biased;
        _ = core_stop.changed() => return,
        _ = public_stop.changed() => {},
        result = start => {
            if let Err(error) = result { log::warn!("Public discovery startup: {error}"); }
            tokio::select! {
                biased;
                _ = core_stop.changed() => return,
                _ = public_stop.changed() => {},
            }
        }
    }
    tokio::select! {
        biased;
        _ = core_stop.changed() => {},
        result = finish => { if let Err(error) = result { log::warn!("Public discovery shutdown: {error}"); } },
    }
}

pub(crate) async fn stop_locked(state: &ServerState, guard: &mut Option<PublicServer>) {
    let _control = state.peer_status.control.lock().await;
    state
        .peer_status
        .public_active
        .store(false, std::sync::atomic::Ordering::SeqCst);
    plain_rs::services::peers::peer_status_controller::stop(state).await;
    if let Err(error) = state.aware.stop(state).await {
        log::warn!("Aware subscription shutdown: {error}");
    }
    state
        .main_ws
        .close_client(&state.subscription_sessions, None);
    state.mms.cancel_all().await;
    let _cast_guard = state.cast.operations.lock().await;
    plain_rs::services::casting::dlna_sender_playback::end(state, true).await;
    let _receiver_guard = state.cast.receiver_operations.lock().await;
    plain_rs::services::casting::receiver::lifecycle::stop_locked(state).await;
    state.cast.stop_tasks();
    plain_rs::services::casting::dlna_sender_runtime::release_permission(
        state,
        &state.cast.scan_lease,
    )
    .await;
    state.cast.publish(state);
    if let Some(public) = guard.take() {
        let _ = public.stop.send(true);
        let PublicServer {
            listeners,
            discovery,
            ..
        } = public;
        discovery.finish().await;
        listeners.shutdown().await;
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/http/listeners/lifecycle.rs"]
mod tests;
