use crate::server::ServerState;
use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;

pub(crate) async fn authorize(
    State(state): State<ServerState>,
    request: Request,
    next: Next,
) -> Response {
    if state.authenticated(request.headers())
        && !request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .is_some_and(|peer| peer.0.ip().is_loopback())
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}

#[cfg(test)]
#[path = "../../../tests/unit/http/auth/local_client.rs"]
mod tests;
