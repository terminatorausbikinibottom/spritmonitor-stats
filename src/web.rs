//! HTTP layer: routing, state and middleware.

pub mod assets;
pub mod error;
pub mod routes;
pub mod session;
pub mod views;

use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub sessions: session::SessionStore,
}

pub fn router(config: Config) -> Router {
    let state = AppState {
        sessions: session::SessionStore::new(config.session_ttl, config.session_capacity),
        config: Arc::new(config),
    };
    // Multipart framing adds overhead on top of the file itself.
    let body_limit = state.config.max_upload_bytes + 64 * 1024;

    Router::new()
        .route("/", get(routes::index))
        .route("/upload", post(routes::upload))
        .route("/stats", get(routes::stats))
        .route("/partials/insurance", get(routes::partial_insurance))
        .route("/partials/forecast", get(routes::partial_forecast))
        .route("/lang/{code}", get(routes::switch_lang))
        .route("/assets/{file}", get(assets::serve))
        .route("/healthz", get(|| async { "ok" }))
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
