use super::PeerContext;
use crate::graphql::resolvers::chat::{ChatItem, item};
use plain_graphql::{Context, Object};
use plain_libs::serde_json::json;
use plain_rs::features::chat::enums::ChannelSystemMessageType;

pub(crate) struct Mutation;

#[Object]
impl Mutation {
    async fn create_chat_item(
        &self,
        ctx: &Context<'_>,
        content: String,
    ) -> plain_graphql::Result<Vec<ChatItem>> {
        let c = ctx.data_unchecked::<PeerContext>();
        plain_libs::serde_json::from_str::<crate::graphql::resolvers::chat::content::ChatContent>(
            &content,
        )?;
        let Some(received) = plain_rs::features::chat::messages::message_lifecycle::receive(
            &c.state.db,
            &c.authenticated.peer.id,
            &c.channel_id,
            &content,
            &c.authenticated.signature_b64,
            c.authenticated.timestamp,
        )?
        else {
            return Ok(vec![]);
        };
        let item = item(&received.chat)?;
        let _ = c
            .state
            .events
            .send(plain_rs::infrastructure::events::WsEvent::broadcast(
                plain_rs::features::chat::events::WS_MESSAGE_CREATED,
                json!([plain_rs::features::chat::service::chat_to_json(
                    &received.chat
                )])
                .to_string(),
            ));
        if let Err(error) = plain_rs::features::services::downloads::chat::received(
            &c.state,
            &received.chat,
            &received.peer.id,
        )
        .await
        {
            log::warn!("Incoming attachment queue: {error}");
        }
        c.state.previews.request(&received.chat.id);
        Ok(vec![item])
    }
    async fn channel_system_message(
        &self,
        ctx: &Context<'_>,
        r#type: ChannelSystemMessageType,
        payload: String,
    ) -> plain_graphql::Result<bool> {
        let c = ctx.data_unchecked::<PeerContext>();
        match plain_rs::features::services::chat::channel_runtime::receive(
            &c.state,
            &c.authenticated.peer.id,
            r#type,
            &payload,
        )
        .await
        {
            Ok(received) => Ok(received["accepted"].as_bool().unwrap_or(false)),
            Err(error) => {
                log::warn!("Channel message rejected: {error}");
                Ok(false)
            }
        }
    }
}
