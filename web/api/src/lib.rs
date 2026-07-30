//! Axum backend for the deckgym web app: accounts, decks, battle history.
//! Auth (Phase 1) is real; deck/game routes below are still stubs establishing the shape
//! described in `web/SPEC.md`.

pub mod auth;
pub mod config;
pub mod error;
pub mod models;

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use config::Config;
use serde_json::{json, Value};
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub http: reqwest::Client,
    pub config: Config,
}

/// Builds the router with all routes wired up, but without the CORS/tracing middleware
/// `main.rs` layers on for real serving — kept out of here so integration tests (`tests/`) can
/// drive routes directly without needing to spoof an `Origin` header.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/decks", get(list_decks_stub))
        .route("/api/games", get(list_games_stub))
        .merge(auth::router())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "status": "ok", "db": "connected" })),
        ),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "error", "db": "unreachable", "detail": e.to_string() })),
        ),
    }
}

// --- Stubs below: establish the route shape from web/SPEC.md; no real logic yet (Phase 2/4). ---

async fn list_decks_stub() -> Json<Value> {
    Json(json!([]))
}

async fn list_games_stub() -> Json<Value> {
    Json(json!([]))
}
