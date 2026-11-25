use crate::infrastructure::transport::contract::{PeerDispatch, PeerTransport};
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
    async fn queued_message(
        &self,
        dispatch: &PeerDispatch<'_>,
        build_request: impl Fn() -> Result<String, String> + Send + Sync,
    ) -> Result<(), String> {
        let response = crate::features::services::peers::peer_transport::send_chat(
            &self.state,
            dispatch,
            build_request,
            self.attempt,
        )
        .await?;
        message_response(&response)
    }
}
pub async fn deliver(
    state: &CoreRuntime,
    id: &str,
) -> plain_libs::anyhow::Result<crate::features::chat::delivery::Receipt> {
    deliver_observed(
        state,
        id,
        None,
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
        let key = crate::features::prefs::security::signing_keypair(&state.prefs)?;
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
                    crate::features::chat::attachments::outgoing::import(
                        &state.db, &state.directory, chat, |uri| async move { attachment_source(state, &uri).await },
                    )
                },
                |chat| {
                    pending(chat);
                    let _ = state
                        .events
                        .send(crate::infrastructure::events::WsEvent::broadcast(
                            crate::features::chat::events::WS_MESSAGE_UPDATED,
                            plain_libs::serde_json::json!([
                                crate::features::chat::service::chat_to_json(chat)
                            ])
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
                plain_libs::serde_json::json!([crate::features::chat::service::chat_to_json(chat)])
                    .to_string(),
            ));
    }
    Ok(receipt)
}

async fn attachment_source(
    state: &CoreRuntime,
    uri: &str,
) -> plain_libs::anyhow::Result<crate::features::chat::attachments::outgoing::Source> {
    use crate::features::chat::attachments::outgoing::Source;
    use crate::features::services::files::{links, source_files};
    let source = links::resolve_native_reference(state, uri).await?;
    if source.path.contains("!zip!/") {
        let path = state.archives.preview(&state.file_platform, source.path).await
            .map_err(plain_libs::anyhow::Error::msg)?;
        return Ok(Source::Local(path));
    }
    if source.path.starts_with("content://") || source.path.starts_with("ph://") {
        let staged = source_files::stage(
            &source.path, state.db.clone(), &state.directory, &state.resources,
            state.cache.root(), state.stop.clone(),
        ).await.map_err(plain_libs::anyhow::Error::msg)?;
        Ok(Source::Staged(staged))
    } else {
        Ok(Source::Local(source.path.into()))
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/chat/chat_delivery.rs"]
mod tests;
