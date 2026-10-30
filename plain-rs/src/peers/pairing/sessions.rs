use crate::crypto::EcdhSession;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(90);
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub device_id: String,
    pub device_name: String,
    pub device_ip: String,
    pub device_port: u16,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticket {
    #[serde(flatten)]
    pub target: Target,
    pub generation: String,
    #[serde(default)]
    pub request_signature: String,
    pub delay_ms: u64,
}
pub struct Pending {
    pub ticket: Ticket,
    pub ecdh: EcdhSession,
    started: Instant,
    sent: bool,
}
#[derive(Default)]
pub struct Sessions(
    Mutex<HashMap<String, Pending>>,
    Mutex<HashMap<String, Incoming>>,
);
struct Incoming {
    signature: String,
    public_key: String,
    target: Target,
    timestamp: i64,
    received: Instant,
}

impl Incoming {
    fn expired(&self) -> bool {
        self.received.elapsed() >= RESPONSE_TIMEOUT
            || !crate::peers::pairing::timestamp_ok(self.timestamp)
    }
}

impl Sessions {
    pub fn receive(&self, request: &crate::peers::pairing::protocol::PairingRequest) -> bool {
        let mut incoming = self.1.lock().unwrap();
        incoming.retain(|_, pending| crate::peers::pairing::timestamp_ok(pending.timestamp));
        if incoming
            .get(&request.from_id)
            .is_some_and(|p| p.timestamp > request.timestamp)
        {
            return false;
        }
        let new = incoming
            .get(&request.from_id)
            .is_none_or(|p| p.signature != request.signature);
        let target = Target {
            device_id: request.from_id.clone(),
            device_name: request.from_name.clone(),
            device_ip: request.from_ip.clone(),
            device_port: request.port,
        };
        if !new && target.device_ip.is_empty() {
            return false;
        }
        let received = incoming
            .get(&request.from_id)
            .filter(|pending| pending.signature == request.signature)
            .map(|pending| pending.received)
            .unwrap_or_else(Instant::now);
        incoming.insert(
            request.from_id.clone(),
            Incoming {
                signature: request.signature.clone(),
                public_key: request.signature_public_key.clone(),
                target,
                timestamp: request.timestamp,
                received,
            },
        );
        new
    }
    pub fn incoming_target(&self, id: &str, signature: &str) -> Option<Target> {
        self.1
            .lock()
            .unwrap()
            .get(id)
            .filter(|pending| pending.signature == signature && !pending.expired())
            .map(|pending| pending.target.clone())
    }
    pub fn take_incoming(&self, id: &str, signature: &str) -> Option<Target> {
        let mut incoming = self.1.lock().unwrap();
        let current = incoming.get(id)?;
        if current.signature != signature || current.expired() {
            return None;
        }
        incoming.remove(id).map(|p| p.target)
    }
    pub fn receive_cancel(
        &self,
        cancel: &crate::peers::pairing::protocol::PairingCancel,
    ) -> Option<Target> {
        let mut incoming = self.1.lock().unwrap();
        let current = incoming.get(&cancel.from_id)?;
        if current.signature != cancel.request_signature
            || current.public_key != cancel.signature_public_key
        {
            return None;
        }
        incoming.remove(&cancel.from_id).map(|p| p.target)
    }
    pub fn bind_request(&self, id: &str, generation: &str, signature: &str) -> Option<Ticket> {
        let mut pending = self.0.lock().unwrap();
        let current = pending
            .get_mut(id)
            .filter(|p| p.ticket.generation == generation)?;
        current.ticket.request_signature = signature.into();
        Some(current.ticket.clone())
    }
    pub fn take_response(
        &self,
        id: &str,
        generation: Option<&str>,
        request_signature: &str,
    ) -> Option<Pending> {
        let mut pending = self.0.lock().unwrap();
        let current = pending.get(id)?;
        if request_signature.is_empty()
            || current.ticket.request_signature != request_signature
            || generation.is_some_and(|g| g != current.ticket.generation)
        {
            return None;
        }
        pending.remove(id)
    }
    pub fn start(&self, target: Target, ecdh: EcdhSession) -> Ticket {
        self.start_generation(target, ecdh, uuid::Uuid::new_v4().to_string())
    }
    pub fn start_generation(
        &self,
        target: Target,
        ecdh: EcdhSession,
        generation: String,
    ) -> Ticket {
        let ticket = Ticket {
            target,
            generation,
            request_signature: String::new(),
            delay_ms: RESPONSE_TIMEOUT.as_millis() as u64,
        };
        self.0.lock().unwrap().insert(
            ticket.target.device_id.clone(),
            Pending {
                ticket: ticket.clone(),
                ecdh,
                started: Instant::now(),
                sent: false,
            },
        );
        ticket
    }
    pub fn contains(&self, id: &str) -> bool {
        self.0.lock().unwrap().contains_key(id)
    }
    pub fn tickets(&self) -> Vec<Ticket> {
        self.0
            .lock()
            .unwrap()
            .values()
            .filter(|pending| !pending.expired())
            .map(|pending| pending.ticket.clone())
            .collect()
    }
    pub fn expire_all(&self) -> Vec<Ticket> {
        let mut pending = self.0.lock().unwrap();
        let ids = pending
            .iter()
            .filter(|(_, value)| value.expired())
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|id| pending.remove(&id).map(|value| value.ticket))
            .collect()
    }
    pub fn current(&self, id: &str, generation: &str) -> bool {
        self.0
            .lock()
            .unwrap()
            .get(id)
            .is_some_and(|p| p.ticket.generation == generation)
    }
    pub fn mark_sent(&self, id: &str, generation: &str) -> bool {
        let mut pending = self.0.lock().unwrap();
        let Some(p) = pending.get_mut(id) else {
            return false;
        };
        if p.ticket.generation != generation || p.expired() {
            return false;
        }
        p.sent = true;
        true
    }
    pub fn states(&self) -> Vec<(Ticket, &'static str)> {
        self.0
            .lock()
            .unwrap()
            .values()
            .filter(|p| !p.expired())
            .map(|p| {
                (
                    p.ticket.clone(),
                    if p.sent { "PAIRING" } else { "STARTING" },
                )
            })
            .collect()
    }
    pub fn remaining_ms(&self, id: &str, generation: &str) -> Option<u64> {
        let pending = self.0.lock().unwrap();
        let p = pending.get(id)?;
        if p.ticket.generation != generation || p.expired() {
            return None;
        }
        Some(
            RESPONSE_TIMEOUT
                .saturating_sub(p.started.elapsed())
                .as_millis()
                .min(u64::MAX as u128) as u64,
        )
    }
    /// Test seam: age an existing ticket so [`Pending::expired`] reports
    /// it as past `RESPONSE_TIMEOUT`. The expiry worker lives in
    /// plain-server, and `#[cfg(test)]` here cannot reach the private
    /// `started` field from that crate's test binary.
    #[cfg(feature = "test-seams")]
    pub fn age_for_test(&self, id: &str, by: Duration) {
        if let Some(pending) = self.0.lock().unwrap().get_mut(id) {
            pending.started -= by;
        }
    }
    #[cfg(feature = "test-seams")]
    pub fn age_incoming_for_test(&self, id: &str, by: Duration) {
        if let Some(incoming) = self.1.lock().unwrap().get_mut(id) {
            incoming.received -= by;
        }
    }
    pub fn take_checked(&self, id: &str, generation: Option<&str>) -> Option<Pending> {
        let mut pending = self.0.lock().unwrap();
        if generation.is_some_and(|g| pending.get(id).is_none_or(|p| p.ticket.generation != g)) {
            return None;
        }
        pending.remove(id)
    }
    pub fn take(&self, id: &str) -> Option<Pending> {
        self.0.lock().unwrap().remove(id)
    }
    pub fn cancel(&self, id: &str, generation: Option<&str>) -> Option<Ticket> {
        let mut pending = self.0.lock().unwrap();
        let item = pending.get(id)?;
        if generation.is_some_and(|g| g != item.ticket.generation) {
            return None;
        }
        pending.remove(id).map(|p| p.ticket)
    }
    pub fn expire(&self, id: &str, generation: &str) -> Option<Ticket> {
        let mut pending = self.0.lock().unwrap();
        let item = pending.get(id)?;
        if item.ticket.generation != generation || !item.expired() {
            return None;
        }
        pending.remove(id).map(|p| p.ticket)
    }
}
impl Pending {
    pub fn expired(&self) -> bool {
        self.started.elapsed() >= RESPONSE_TIMEOUT
    }
}
#[cfg(test)]
#[path = "../../../tests/unit/peers/pairing/sessions.rs"]
mod tests;
