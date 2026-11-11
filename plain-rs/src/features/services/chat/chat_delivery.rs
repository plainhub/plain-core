use crate::infrastructure::db::DPeer;
use crate::infrastructure::transport::contract::PeerTransport;
use crate::infrastructure::transport::contract::message_response;
use crate::runtime::Runtime as CoreRuntime;
pub struct Transport {
    state: CoreRuntime,
    attempt: crate::infrastructure::transport::router::Attempt,
}
impl PeerTransport for Transport {
    async fn post<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        _: Option<&'a str>,
        _: &'a [u8],
    ) -> Result<Vec<u8>, String> {
        Err("Raw mobile HTTP transport unavailable".into())
    }
    async fn message(
        &self,
        peer: &DPeer,
        _: &str,
        channel_id: &str,
        key: &[u8],
        body: &str,
    ) -> Result<(), String> {
        let response = crate::features::services::peers::peer_transport::send_with_attempt(
            &self.state,
            peer,
            channel_id,
            key,
            body,
            self.attempt,
        )
        .await?;
        message_response(&response)
    }
}
pub async fn deliver(
    state: &CoreRuntime,
    id: &str,
    recipients: Option<Vec<String>>,
) -> plain_libs::anyhow::Result<crate::features::chat::delivery::Receipt> {
    deliver_observed(
        state,
        id,
        recipients,
        crate::infrastructure::transport::router::Attempt::Automatic,
        |_| {},
    )
    .await
}
pub async fn deliver_observed(
    state: &CoreRuntime,
    id: &str,
    recipients: Option<Vec<String>>,
    attempt: crate::infrastructure::transport::router::Attempt,
    pending: impl Fn(&crate::infrastructure::db::DChat) + Send + Sync,
) -> plain_libs::anyhow::Result<crate::features::chat::delivery::Receipt> {
    let result = async {
        let client_id = state
            .native_client_id()
            .map_err(plain_libs::anyhow::Error::msg)?;
        let key = crate::features::services::peers::peer_wire::signing_keypair(&state.prefs)?;
        state
            .delivery
            .send_observed(
                &Transport {
                    state: state.clone(),
                    attempt,
                },
                &client_id,
                &key,
                id,
                recipients,
                |chat| {
                    pending(chat);
                    let _ = state
                        .events
                        .send(crate::infrastructure::events::WsEvent::broadcast(
                            crate::features::chat::events::WS_MESSAGE_UPDATED,
                            serde_json::json!([crate::features::chat::service::chat_to_json(chat)])
                                .to_string(),
                        ));
                },
            )
            .await
    }
    .await;
    let receipt = result?;
    if receipt.rediscover {
        state.mdns.browse_resident();
    }
    if let Some(chat) = &receipt.chat {
        let _ = state
            .events
            .send(crate::infrastructure::events::WsEvent::broadcast(
                crate::features::chat::events::WS_MESSAGE_UPDATED,
                serde_json::json!([crate::features::chat::service::chat_to_json(chat)]).to_string(),
            ));
    }
    Ok(receipt)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/chat/chat_delivery.rs"]
mod tests;
