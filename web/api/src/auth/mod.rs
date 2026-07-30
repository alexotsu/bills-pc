pub mod handlers;
pub mod oauth;
pub mod password;
pub mod session;

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/register", post(handlers::register))
        .route("/api/auth/login", post(handlers::login))
        .route("/api/auth/logout", post(handlers::logout))
        .route("/api/auth/me", get(handlers::me))
        .route("/api/auth/account", delete(handlers::delete_account))
        .route(
            "/api/auth/oauth/:provider/start",
            get(handlers::oauth_start),
        )
        .route(
            "/api/auth/oauth/:provider/callback",
            get(handlers::oauth_callback),
        )
        .route(
            "/api/auth/oauth/complete",
            post(handlers::complete_oauth_signup),
        )
}
