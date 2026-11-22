use super::{ManagedPeerRoutes, PeerRoute};
use crate::features::peers::error::PeerConnectionErrorCode;
use crate::infrastructure::db::DPeer;
use crate::runtime::Runtime;
use plain_libs::anyhow::{Result, anyhow, ensure};
use plain_libs::protocols::http_client;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::watch, time::Instant};

pub(in crate::features::services::peers) const RECOVERY_TIMEOUT: Duration = Duration::from_secs(10);
pub(in crate::features::services::peers) type Outcome =
    Option<Result<Arc<PeerRoute>, Arc<plain_libs::anyhow::Error>>>;

#[derive(Default)]
pub(in crate::features::services::peers) struct Recovery(Mutex<HashMap<String, Round>>);

struct Round {
    peer: DPeer,
    result: watch::Receiver<Outcome>,
}

impl Recovery {
    pub(in crate::features::services::peers) fn request(
        &self,
        state: &Runtime,
        failed: &DPeer,
    ) -> Result<watch::Receiver<Outcome>> {
        current(state, failed)?;
        let mut rounds = self.0.lock().unwrap();
        if let Some(round) = rounds.get(&failed.id) {
            ensure!(
                same_session(&round.peer, failed),
                "Management session changed"
            );
            return Ok(round.result.clone());
        }

        let (publish, receive) = watch::channel(None);
        rounds.insert(
            failed.id.clone(),
            Round {
                peer: failed.clone(),
                result: receive.clone(),
            },
        );
        let state = state.clone();
        let failed = failed.clone();
        tokio::spawn(async move {
            let mut stop = state.stop.clone();
            let outcome = tokio::select! {
                biased;
                _ = stop.wait_for(|value| *value) => Err(anyhow!("Server stopped")),
                result = tokio::time::timeout(RECOVERY_TIMEOUT, find_route(&state, &failed)) => {
                    result.unwrap_or_else(|_| Err(PeerConnectionErrorCode::ConnectionTimeout.into()))
                }
            };
            let mut rounds = state.managed_resources.recovery.0.lock().unwrap();
            publish.send_replace(Some(outcome.map_err(Arc::new)));
            rounds.remove(&failed.id);
        });
        Ok(receive)
    }
}

pub(in crate::features::services::peers) fn current(
    state: &Runtime,
    expected: &DPeer,
) -> Result<DPeer> {
    let peer = crate::infrastructure::db::chat_store::peers::get(&state.db, &expected.id)?
        .ok_or(PeerConnectionErrorCode::PeerRemoved)?;
    ensure!(same_session(&peer, expected), "Management session revoked");
    Ok(peer)
}

fn same_session(a: &DPeer, b: &DPeer) -> bool {
    a.is_paired() && a.id == b.id && a.token == b.token && a.public_key == b.public_key
}

pub(in crate::features::services::peers) fn moved(a: &DPeer, b: &DPeer) -> bool {
    a.ip != b.ip || a.port != b.port
}

async fn find_route(state: &Runtime, failed: &DPeer) -> Result<Arc<PeerRoute>> {
    let mut updated = state.db.peer_changes();
    let mut browsed = false;
    loop {
        let candidate = current(state, failed)?;
        if moved(&candidate, failed) {
            let result = state.managed_resources.route(state, &candidate.id).await;
            let latest = current(state, failed)?;
            if moved(&latest, &candidate) {
                continue;
            }
            let route = result?;
            ensure!(
                same_session(&route.peer, failed) && moved(&route.peer, failed),
                "Management route changed"
            );
            return Ok(route);
        }
        if !browsed {
            state.mdns.browse_resident();
            browsed = true;
        }
        updated.changed().await?;
    }
}

impl ManagedPeerRoutes {
    pub async fn exchange(
        &self,
        state: &Runtime,
        target: &str,
        request: &str,
        deadline: Instant,
        safe_read: bool,
        authorized: impl Fn() -> Result<()>,
    ) -> Result<Vec<u8>> {
        let failed = self.peer(state, target)?;
        let remaining = || deadline.saturating_duration_since(Instant::now());
        let first = async {
            let route = self.route(state, target).await?;
            ensure!(
                same_session(&route.peer, &failed),
                "Management session revoked"
            );
            authorized()?;
            route.exchange(state, request, Some(remaining())).await
        }
        .await;
        let Err(error) = first else { return first };
        if error.is::<crate::features::access_error::AccessErrorCode>() || tls_rejection(&error) {
            return Err(error);
        }
        let latest = current(state, &failed)?;
        let connection_error = error
            .downcast_ref::<http_client::Error>()
            .is_some_and(|error| error.is_connect() || error.is_timeout());
        let address_error = matches!(
            error.downcast_ref::<PeerConnectionErrorCode>(),
            Some(PeerConnectionErrorCode::PeerAddressUnavailable)
        );
        if !(connection_error || address_error || moved(&latest, &failed)) {
            return Err(error);
        }

        let mut result = self.recovery.request(state, &failed)?;
        if !safe_read {
            return Err(error);
        }
        let route = wait(&mut result, deadline).await?;
        authorized()?;
        route.exchange(state, request, Some(remaining())).await
    }
}

pub(in crate::features::services::peers) async fn wait(
    result: &mut watch::Receiver<Outcome>,
    deadline: Instant,
) -> Result<Arc<PeerRoute>> {
    let ready = tokio::time::timeout_at(deadline, result.wait_for(Option::is_some))
        .await
        .map_err(|_| PeerConnectionErrorCode::ConnectionTimeout)??;
    match ready.as_ref().expect("completed recovery") {
        Ok(route) => Ok(route.clone()),
        Err(error) if error.is::<crate::features::access_error::AccessErrorCode>() => Err((*error
            .downcast_ref::<crate::features::access_error::AccessErrorCode>()
            .unwrap())
        .into()),
        Err(error) => Err(anyhow!(error.to_string())),
    }
}

pub(in crate::features::services::peers) fn tls_rejection(
    error: &plain_libs::anyhow::Error,
) -> bool {
    error.chain().any(|cause| {
        cause.is::<rustls::Error>()
            || cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::InvalidData)
    })
}
