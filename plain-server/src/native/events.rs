use crate::server::ServerState;
use plain_rs::events::WsEvent;
use tokio::sync::broadcast;

pub struct NativeEventStream {
    state: ServerState,
    events: broadcast::Receiver<WsEvent>,
    client_id: String,
    scope: plain_rs::host::request_scope::NativeRequestScope,
}

impl NativeEventStream {
    pub(crate) fn scoped(
        state: ServerState,
        client_id: String,
        scope: plain_rs::host::request_scope::NativeRequestScope,
    ) -> Result<Self, String> {
        let key = plain_rs::utils::base64::base64_decode(&state.native_url_token()?);
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| "local event token unavailable".to_owned())?;
        let events = state.events.subscribe();
        Ok(Self {
            state,
            events,
            client_id,
            scope,
        })
    }

    pub async fn next_ui_frame(&mut self) -> Result<Option<Vec<u8>>, String> {
        let mut stop = self.state.stop.clone();
        let scope = self.scope.clone();
        let closed = scope.wait_closed();
        tokio::pin!(closed);
        loop {
            if *stop.borrow() {
                return Ok(None);
            }
            let event = tokio::select! {
                biased;
                _ = stop.wait_for(|stopped| *stopped) => return Ok(None),
                _ = &mut closed => return Ok(None),
                result = self.events.recv() => match result {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return Ok(None),
                },
            };
            if event.is_host_event()
                || event
                    .target_cid
                    .as_ref()
                    .is_some_and(|target| target != &self.client_id)
            {
                continue;
            }
            if let Some(payload) = event.binary_payload {
                return plain_rs::protocols::websocket::encode_raw(event.event_type, &payload)
                    .map(Some)
                    .ok_or_else(|| "invalid native event frame".into());
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/native/events.rs"]
mod tests;
