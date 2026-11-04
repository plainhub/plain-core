use crate::infrastructure::db::DPeer;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransportType {
    Lan,
    Aware,
    Ble,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Attempt {
    #[default]
    Automatic,
    Retry,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ticket {
    pub id: String,
    pub generation: u64,
    pub transport: TransportType,
}
#[derive(Serialize)]
pub struct Step {
    pub ticket: Option<Ticket>,
    pub error: Option<String>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Outcome {
    Connected,
    Unavailable { error: String },
}
struct Circuit {
    failures: BTreeSet<u64>,
    success: u64,
    opened: Option<Instant>,
    seen: Instant,
}
struct Route {
    peer: String,
    candidates: Vec<TransportType>,
    cursor: usize,
    active: Option<Ticket>,
    errors: Vec<String>,
    deadline: Instant,
    attempt: Attempt,
}
#[derive(Default)]
struct State {
    next: u64,
    circuits: HashMap<(String, TransportType), Circuit>,
    routes: HashMap<String, Route>,
}
#[derive(Default)]
pub struct Router {
    state: Mutex<State>,
}
impl Router {
    pub fn begin(&self, peer: &DPeer, available: &[TransportType]) -> Result<Step> {
        self.begin_with_attempt(peer, available, Attempt::Automatic)
    }
    pub fn begin_with_attempt(
        &self,
        peer: &DPeer,
        available: &[TransportType],
        attempt: Attempt,
    ) -> Result<Step> {
        self.begin_attempt_at(peer, available, attempt, Instant::now())
    }
    #[cfg(test)]
    fn begin_at(&self, peer: &DPeer, available: &[TransportType], now: Instant) -> Result<Step> {
        self.begin_attempt_at(peer, available, Attempt::Automatic, now)
    }
    fn begin_attempt_at(
        &self,
        peer: &DPeer,
        available: &[TransportType],
        attempt: Attempt,
        now: Instant,
    ) -> Result<Step> {
        if peer.id.is_empty() {
            bail!("Missing peer");
        }
        let mut state = self.state.lock().unwrap();
        state.routes.retain(|_, route| route.deadline > now);
        state.circuits.retain(|_, circuit| {
            now.saturating_duration_since(circuit.seen) < Duration::from_secs(600)
        });
        if state.routes.len() >= 64 {
            bail!("Peer transport capacity exceeded");
        }
        let mut route = Route {
            peer: peer.id.clone(),
            candidates: [TransportType::Lan, TransportType::Aware, TransportType::Ble]
                .into_iter()
                .filter(|kind| {
                    available.contains(kind)
                        && (*kind != TransportType::Lan || !peer.ip.trim().is_empty())
                })
                .collect(),
            cursor: 0,
            active: None,
            errors: Vec::new(),
            deadline: now + Duration::from_secs(300),
            attempt,
        };
        let id = plain_libs::utils::short_uuid::short_uuid();
        if state.routes.contains_key(&id) {
            bail!("Transport route ID collision");
        }
        let step = next(&mut state, &mut route, &id, now)?;
        if step.ticket.is_some() {
            state.routes.insert(id, route);
        }
        Ok(step)
    }
    pub fn finish(&self, ticket: &Ticket, outcome: Outcome) -> Result<Step> {
        self.finish_at(ticket, outcome, Instant::now())
    }
    fn finish_at(&self, ticket: &Ticket, outcome: Outcome, now: Instant) -> Result<Step> {
        let mut state = self.state.lock().unwrap();
        let mut route = state
            .routes
            .remove(&ticket.id)
            .ok_or_else(|| anyhow::anyhow!("Transport attempt unavailable"))?;
        if route.active.as_ref() != Some(ticket) {
            state.routes.insert(ticket.id.clone(), route);
            bail!("Stale transport receipt");
        }
        if route.deadline <= now {
            bail!("Transport route expired");
        }
        let circuit = state
            .circuits
            .entry((route.peer.clone(), ticket.transport))
            .or_insert_with(|| Circuit {
                failures: BTreeSet::new(),
                success: 0,
                opened: None,
                seen: now,
            });
        circuit.seen = now;
        match outcome {
            Outcome::Connected => {
                circuit.success = circuit.success.max(ticket.generation);
                circuit
                    .failures
                    .retain(|generation| *generation > circuit.success);
                if circuit.failures.len() < 2 {
                    circuit.opened = None;
                }
                Ok(Step {
                    ticket: None,
                    error: None,
                })
            }
            Outcome::Unavailable { error } => {
                if ticket.generation > circuit.success {
                    circuit.failures.insert(ticket.generation);
                    while circuit.failures.len() > 2 {
                        circuit.failures.pop_first();
                    }
                    if circuit.failures.len() >= 2 {
                        circuit.opened = Some(now);
                    }
                }
                route
                    .errors
                    .push(format!("{:?}: {error}", ticket.transport));
                let step = next(&mut state, &mut route, &ticket.id, now)?;
                if step.ticket.is_some() {
                    state.routes.insert(ticket.id.clone(), route);
                }
                Ok(step)
            }
        }
    }
    pub fn forget(&self, peer: &str) {
        let mut state = self.state.lock().unwrap();
        state.routes.retain(|_, route| route.peer != peer);
        state.circuits.retain(|(id, _), _| id != peer);
    }
    pub fn active(&self) -> HashMap<String, TransportType> {
        let state = self.state.lock().unwrap();
        let now = Instant::now();
        let mut active = HashMap::<String, (u64, TransportType)>::new();
        for route in state.routes.values().filter(|route| route.deadline > now) {
            if let Some(ticket) = &route.active {
                let entry = active
                    .entry(route.peer.clone())
                    .or_insert((ticket.generation, ticket.transport));
                if ticket.generation > entry.0 {
                    *entry = (ticket.generation, ticket.transport);
                }
            }
        }
        active
            .into_iter()
            .map(|(peer, (_, kind))| (peer, kind))
            .collect()
    }
    pub fn abort(&self, ticket: &Ticket) {
        let mut state = self.state.lock().unwrap();
        if state
            .routes
            .get(&ticket.id)
            .is_some_and(|route| route.active.as_ref() == Some(ticket))
        {
            state.routes.remove(&ticket.id);
        }
    }
}
fn next(state: &mut State, route: &mut Route, id: &str, now: Instant) -> Result<Step> {
    while let Some(kind) = route.candidates.get(route.cursor).copied() {
        route.cursor += 1;
        if let Some(circuit) = state.circuits.get_mut(&(route.peer.clone(), kind)) {
            if let Some(opened) = circuit.opened {
                if now.saturating_duration_since(opened) <= Duration::from_secs(30) {
                    if route.attempt == Attempt::Automatic {
                        continue;
                    }
                } else {
                    circuit.opened = None;
                    circuit.failures.clear();
                }
            }
        }
        state.next = state
            .next
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Transport generation overflow"))?;
        let ticket = Ticket {
            id: id.into(),
            generation: state.next,
            transport: kind,
        };
        route.active = Some(ticket.clone());
        return Ok(Step {
            ticket: Some(ticket),
            error: None,
        });
    }
    Ok(Step {
        ticket: None,
        error: Some(if route.errors.is_empty() {
            "All peer transports unavailable or temporarily blocked".into()
        } else {
            route.errors.join("; ")
        }),
    })
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/transport/router.rs"]
mod tests;
