use plain_graphql::{Request, Response};
use plain_libs::serde_json::Value;
use serde::{Deserialize, Serialize};
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
        payload: Vec<plain_graphql::ServerError>,
    },
    Complete {
        id: String,
    },
    Pong {
        payload: Option<Value>,
    },
}
