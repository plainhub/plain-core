#[cfg(feature = "core")]
use crate::assembly::server::ServerState;
use axum::{Router, routing::get};
#[cfg(feature = "core")]
use axum::{extract::DefaultBodyLimit, routing::{post, any}};
#[cfg(feature = "core")]
use tokio::sync::watch;

#[cfg(feature = "core")]
pub(crate) fn build_router(state: ServerState, receiver: watch::Receiver<bool>) -> Router {
    let transfer_routes = transfer_routes(state.clone());
    let status_router = Router::new()
        .route("/status", get(crate::http::websocket::status::public))
        .with_state(crate::http::websocket::status::PublicState {
            state: state.clone(),
            stop: receiver.clone(),
        });
    let nearby_router = Router::new()
        .route("/nearby", post(crate::http::nearby::call))
        .with_state(state.clone());
    let main_graphql_router = Router::new()
        .route(
            "/graphql",
            post(crate::http::graphql::call).get(crate::http::websocket::graphql::upgrade),
        )
        .route("/health", get(crate::http::health::health))
        .route("/proxyfs", get(crate::http::proxy::call))
        .route("/zip/dir", get(crate::http::zip::dir))
        .route("/zip/files", get(crate::http::zip::files))
        .route(
            "/upload",
            post(crate::http::upload::upload)
                .layer(DefaultBodyLimit::max(15 * 60 * 1000 * 1000)),
        )
        .route(
            "/upload_chunk",
            post(crate::http::upload::upload_chunk)
                .layer(DefaultBodyLimit::max(15 * 60 * 1000 * 1000)),
        )
        .route("/auth", get(crate::http::websocket::auth::upgrade))
        .route("/screen", get(crate::http::websocket::screen::upgrade))
        .route("/image_editor", get(crate::http::websocket::image_editor::upgrade))
        .route(
            "/media/:id",
            get(crate::http::dlna::media),
        )
        .route(
            "/callback/cast",
            axum::routing::any(crate::http::dlna::callback),
        )
        .route("/init", post(crate::http::init::init))
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
        .with_state(state.clone());
    Router::new()
        .fallback(crate::http::static_files::fallback)
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024 * 1024))
        .with_state(state.clone())
        .merge(transfer_routes)
        .merge(status_router)
        .merge(nearby_router)
        .merge(main_graphql_router)
        .merge(dlna_routes(state.clone()))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::http::auth::local_client::authorize,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::http::auth::cors::apply,
        ))
}

#[cfg(feature = "core")]
pub(crate) fn transfer_routes(state: ServerState) -> Router {
    Router::new()
        .route("/peer_graphql", post(crate::http::graphql::peer))
        .route(
            "/guest_graphql",
            post(crate::http::graphql::guest),
        )
        .route("/fs", get(crate::http::fs::get))
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
        .with_state(state)
}

#[cfg(feature = "core")]
fn dlna_routes(state: ServerState) -> Router {
    Router::new()
        .route("/description.xml", get(crate::http::dlna::receiver))
        .route("/AVTransport/scpd.xml", get(crate::http::dlna::receiver))
        .route("/RenderingControl/scpd.xml", get(crate::http::dlna::receiver))
        .route("/AVTransport/control", post(crate::http::dlna::receiver))
        .route("/RenderingControl/control", post(crate::http::dlna::receiver))
        .route("/AVTransport/event", any(crate::http::dlna::receiver))
        .route("/RenderingControl/event", any(crate::http::dlna::receiver))
        .with_state(state)
}

#[cfg(feature = "http_transport")]
pub(crate) fn with_readiness(router: Router) -> Router {
    router.route("/__plain_server_ready", get(crate::http::health::ready))
}
