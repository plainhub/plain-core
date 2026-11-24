pub mod mutation;
pub mod query;

use super::ChatItem;
use plain_graphql::{ID, SimpleObject};

#[derive(SimpleObject)]
pub struct ChatConversation {
    pub id: ID,
    pub chat: ChatItem,
}

#[derive(SimpleObject)]
pub struct ChatTextEdit {
    pub changed: bool,
    pub chat: ChatItem,
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-server/unit/graphql/resolvers/chat/host/mod.rs"]
mod tests;
