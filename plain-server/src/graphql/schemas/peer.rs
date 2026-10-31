use crate::graphql::resolvers::peers::query::Query;
use crate::server::ServerState;
use async_graphql::{Context, EmptySubscription, ID, Object, Schema, SimpleObject};
use plain_rs::chat::enums::ChannelSystemMessageType;
use plain_rs::chat::enums::ChatStatus;
use plain_rs::db::DChat;
use plain_rs::transport::auth::AuthenticatedPeer;
use plain_rs::types::Instant;
use serde_json::{Value, json};

#[derive(SimpleObject)]
struct ChatItem {
    from_id: ID,
    to_id: ID,
    channel_id: Option<ID>,
    id: ID,
    content: String,
    created_at: Instant,
    updated_at: Instant,
    status: ChatStatus,
    status_data: String,
}
impl TryFrom<DChat> for ChatItem {
    type Error = anyhow::Error;
    fn try_from(row: DChat) -> anyhow::Result<Self> {
        Ok(Self {
            from_id: ID(row.from_id),
            to_id: ID(row.to_id),
            channel_id: row.channel_id.map(ID),
            id: ID(row.id),
            content: row.content,
            created_at: Instant(chrono::DateTime::parse_from_rfc3339(&row.created_at)?.into()),
            updated_at: Instant(chrono::DateTime::parse_from_rfc3339(&row.updated_at)?.into()),
            status: row.status,
            status_data: row.status_data.unwrap_or_default(),
        })
    }
}
struct PeerContext {
    state: ServerState,
    authenticated: AuthenticatedPeer,
    channel_id: String,
}
pub(crate) struct Mutation;
#[Object]
impl Mutation {
    async fn create_chat_item(
        &self,
        ctx: &Context<'_>,
        content: String,
    ) -> async_graphql::Result<Vec<ChatItem>> {
        let c = ctx.data_unchecked::<PeerContext>();
        let Some(received) = plain_rs::chat::messages::message_lifecycle::receive(
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
        let item = ChatItem::try_from(received.chat.clone())?;
        let _ = c.state.events.send(plain_rs::events::WsEvent::broadcast(
            plain_rs::chat::events::WS_MESSAGE_CREATED,
            json!([plain_rs::chat::service::chat_to_json(&received.chat)]).to_string(),
        ));
        if let Ok(content) = serde_json::from_str::<Value>(&received.chat.content) {
            if matches!(content["type"].as_str(), Some("FILES" | "IMAGES")) {
                if let Some(files) = content["value"]["items"].as_array() {
                    for file in files {
                        if let Some(id) = file["id"].as_str() {
                            if let Err(error) =
                                c.state
                                    .downloads
                                    .enqueue(&received.chat.id, id, &received.peer.id)
                            {
                                log::warn!("Incoming attachment queue: {error}");
                            }
                        }
                    }
                }
            }
        }
        c.state.previews.request(&received.chat.id);
        Ok(vec![item])
    }
    async fn channel_system_message(
        &self,
        ctx: &Context<'_>,
        r#type: ChannelSystemMessageType,
        payload: String,
    ) -> async_graphql::Result<bool> {
        let c = ctx.data_unchecked::<PeerContext>();
        match plain_rs::services::chat::channel_runtime::receive(
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
    async fn start_aware(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        let c = ctx.data_unchecked::<PeerContext>();
        Ok(c.state
            .aware
            .subscribe(&c.state, &c.authenticated.peer.id)
            .await?)
    }
}
pub(crate) type PeerSchema = Schema<Query, Mutation, EmptySubscription>;
pub(crate) fn schema() -> PeerSchema {
    Schema::build(Query, Mutation, EmptySubscription)
        .extension(crate::graphql::enum_literals::EnumLiteralValidation)
        .register_output_type::<plain_rs::types::Long>()
        .register_output_type::<plain_rs::chat::enums::PeerStatus>()
        .limit_depth(16)
        .limit_complexity(256)
        .finish()
}
pub(crate) async fn execute(
    state: &ServerState,
    client_id: &str,
    channel_id: &str,
    body: &[u8],
) -> (u16, Vec<u8>) {
    let authenticated =
        match plain_rs::transport::auth::authenticate(&state.db, client_id, channel_id, body) {
            Ok(authenticated) => authenticated,
            Err(error) => return (error.http_status(), vec![]),
        };
    let request = match serde_json::from_str::<async_graphql::Request>(&authenticated.graphql_json)
    {
        Ok(request) => request,
        Err(_) => return (400, vec![]),
    };
    let key = authenticated.key.clone();
    let response = state
        .peer_schema
        .execute(request.data(PeerContext {
            state: state.clone(),
            authenticated,
            channel_id: channel_id.into(),
        }))
        .await;
    let context = plain_rs::transport::http_response::Context::new("/peer_graphql", body);
    match serde_json::to_vec(&response) {
        Ok(body) => context
            .encrypt(&key, &body)
            .map(|body| (200, body))
            .unwrap_or((500, vec![])),
        Err(_) => (500, vec![]),
    }
}
#[cfg(test)]
#[path = "../../../tests/unit/graphql/schemas/peer_host.rs"]
mod tests;
