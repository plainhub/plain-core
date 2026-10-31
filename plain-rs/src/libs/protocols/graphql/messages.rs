use async_graphql::{Request, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMessage {
    Subscribe { id: String, payload: Request },
    Complete { id: String },
    Ping { payload: Option<Value> },
    Pong { payload: Option<Value> },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Next {
        id: String,
        payload: Response,
    },
    Error {
        id: String,
        payload: Vec<async_graphql::ServerError>,
    },
    Complete {
        id: String,
    },
    Pong {
        payload: Option<Value>,
    },
}
