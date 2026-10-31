use super::runtime::{self, Channel, Params};
use crate::server::ServerState;
use axum::{
    extract::{ConnectInfo, Query, State, WebSocketUpgrade},
    response::Response,
};
use std::net::SocketAddr;

pub(crate) async fn upgrade(
    State(state): State<ServerState>,
    Query(params): Query<Params>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
) -> Response {
    runtime::upgrade(state, params, remote, ws, Channel::Screen)
}
