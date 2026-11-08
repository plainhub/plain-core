use super::runtime::{self, Channel, Params};
use crate::server::ServerState;
use axum::{
    extract::{ConnectInfo, Query, State},
    response::Response,
};
use plain_libs::protocols::websocket::WebSocketUpgrade;
use std::net::SocketAddr;

pub(crate) async fn upgrade(
    State(state): State<ServerState>,
    Query(params): Query<Params>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
) -> Response {
    runtime::upgrade(state, params, remote, ws, Channel::ImageEditor)
}
