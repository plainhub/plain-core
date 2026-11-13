use super::*;
use crate::features::files::resource_link::{self, Scope};
use crate::features::services::files::content::Action;
use crate::features::services::peers::{managed_resources::PeerRoute, peer_address};
use crate::infrastructure::db::{DPeer, chat_store::peers};
use std::sync::Arc;

fn scope(url: &Url) -> Result<Scope> {
    let id = url
        .query_pairs()
        .find(|(key, _)| key == "id")
        .expect("Resource URL validated")
        .1
        .into_owned();
    Ok(resource_link::parse(&id)?.0)
}

fn origin(peer: &DPeer) -> Option<plain_libs::protocols::http_client::Url> {
    peer.base_url().and_then(|base| Url::parse(&base).ok())
}

pub(crate) fn prepare(state: &CoreRuntime, urls: &[String]) -> Result<Vec<DPeer>> {
    let peers = peers::all(&state.db)?;
    let mut selected: Vec<DPeer> = vec![];
    for raw in urls {
        let url = super::url(state, raw)?;
        if is_local(state, &url)? || matches!(scope(&url)?, Scope::Share(_)) {
            continue;
        }
        let candidates: Vec<_> = peers
            .iter()
            .filter(|peer| {
                peer.is_paired() && origin(peer).is_some_and(|base| base.origin() == url.origin())
            })
            .collect();
        ensure!(
            candidates.len() == 1,
            "Resource device is unavailable or ambiguous"
        );
        let peer = candidates[0];
        if !selected.iter().any(|saved| saved.id == peer.id) {
            selected.push(peer.clone());
        }
    }
    Ok(selected)
}

pub(crate) struct Read {
    pub response: plain_libs::protocols::http_client::Response,
    pub route: Option<Arc<PeerRoute>>,
}

pub(crate) async fn get(
    state: &CoreRuntime,
    raw: &str,
    action: Action,
    expected: &[DPeer],
) -> Result<Read> {
    let url = super::url(state, raw)?;
    let id = url
        .query_pairs()
        .find(|(key, _)| key == "id")
        .expect("Resource URL validated")
        .1
        .into_owned();
    let (base, route) = match scope(&url)? {
        Scope::Share(_) => (url.clone(), None),
        Scope::Session(owner) => {
            let peer = expected
                .iter()
                .find(|peer| origin(peer).is_some_and(|base| base.origin() == url.origin()))
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Resource device unavailable"))?;
            let current = peer_address::current_identity(&state.db, &peer.id, peer)?;
            ensure!(
                current.is_paired() && current.token == peer.token,
                "Management session revoked"
            );
            let route = state.managed_resources.route(state, &peer.id).await?;
            ensure!(
                owner == route.client_id,
                "Resource session does not belong to this device"
            );
            (route.base.join("/fs")?, Some(route))
        }
    };
    let mut target = base;
    target.set_query(None);
    target
        .query_pairs_mut()
        .append_pair("id", &id)
        .append_pair("action", action.as_str());
    let request = match &route {
        Some(route) => route
            .client
            .get(target)
            .header("c-id", &route.client_id)
            .header(
                crate::features::services::peers::managed_resources::MANAGED_RESOURCE,
                "1",
            )
            .bearer_auth(&route.peer.token),
        None => state.lan.client.get(target),
    };
    let response = request.timeout(Duration::from_secs(15 * 60)).send().await?;
    ensure!(
        response.status().is_success(),
        "Resource HTTP {}",
        response.status()
    );
    if let Some(route) = &route {
        route.check_current(state)?;
    }
    Ok(Read { response, route })
}
