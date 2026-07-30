//! Integration tests for the auth routes, driven through the real Axum router (`api::app`) —
//! not by calling handler functions directly — so these exercise the same request/response path
//! a browser would. Each test gets its own throwaway Postgres database via `#[sqlx::test]`
//! (migrations from `api/migrations` applied automatically), so tests can run in parallel and
//! don't need any manual DB setup beyond a reachable Postgres (see `web/dev.sh`).

use api::{app, config::Config, config::OAuthConfig, AppState};
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

fn test_config() -> Config {
    Config {
        database_url: String::new(), // unused; #[sqlx::test] supplies the pool directly
        oauth: OAuthConfig {
            google_client_id: "test-google-client-id".to_string(),
            google_client_secret: "test-google-client-secret".to_string(),
            facebook_client_id: String::new(),
            facebook_client_secret: String::new(),
            api_base_url: "http://localhost:8080".to_string(),
        },
        frontend_url: "http://localhost:3000".to_string(),
        cookie_secure: false,
    }
}

fn test_app(pool: PgPool) -> axum::Router {
    app(AppState {
        db: pool,
        http: reqwest::Client::new(),
        config: test_config(),
    })
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Pulls just the `name=value` pair out of a `Set-Cookie` response header, discarding the
/// attributes (`HttpOnly`, `Path`, ...), so it can be replayed as a request's `Cookie` header.
fn session_cookie(response: &axum::response::Response) -> String {
    let raw = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("expected a Set-Cookie header")
        .to_str()
        .unwrap();
    raw.split(';').next().unwrap().to_string()
}

fn post_json(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(uri: &str, cookie: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method("GET").uri(uri);
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    builder.body(Body::empty()).unwrap()
}

const VALID_REGISTER_BODY: fn(&str) -> Value = |email| {
    json!({
        "email": email,
        "password": "correct horse battery staple",
        "training_data_opt_in": true,
    })
};

#[sqlx::test]
async fn register_creates_account_and_sets_session_cookie(pool: PgPool) {
    let app = test_app(pool);

    let response = app
        .oneshot(post_json(
            "/api/auth/register",
            VALID_REGISTER_BODY("new@example.com"),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(session_cookie(&response).starts_with("deckgym_session="));

    let body = json_body(response).await;
    assert_eq!(body["email"], "new@example.com");
    assert_eq!(body["training_data_opt_in"], true);
    // password_hash is `#[serde(skip_serializing)]` on User — must never reach the client.
    assert!(body.get("password_hash").is_none());
}

#[sqlx::test]
async fn register_without_opt_in_is_rejected(pool: PgPool) {
    let app = test_app(pool);

    let response = app
        .oneshot(post_json(
            "/api/auth/register",
            json!({
                "email": "nope@example.com",
                "password": "correct horse battery staple",
                "training_data_opt_in": false,
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn register_with_short_password_is_rejected(pool: PgPool) {
    let app = test_app(pool);

    let response = app
        .oneshot(post_json(
            "/api/auth/register",
            json!({
                "email": "short@example.com",
                "password": "short",
                "training_data_opt_in": true,
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn registering_the_same_email_twice_conflicts(pool: PgPool) {
    let app = test_app(pool);

    let first = app
        .clone()
        .oneshot(post_json(
            "/api/auth/register",
            VALID_REGISTER_BODY("dupe@example.com"),
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .oneshot(post_json(
            "/api/auth/register",
            VALID_REGISTER_BODY("dupe@example.com"),
        ))
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CONFLICT);
}

#[sqlx::test]
async fn login_with_correct_password_succeeds(pool: PgPool) {
    let app = test_app(pool);

    app.clone()
        .oneshot(post_json(
            "/api/auth/register",
            VALID_REGISTER_BODY("login@example.com"),
        ))
        .await
        .unwrap();

    let response = app
        .oneshot(post_json(
            "/api/auth/login",
            json!({
                "email": "login@example.com",
                "password": "correct horse battery staple",
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(session_cookie(&response).starts_with("deckgym_session="));
}

#[sqlx::test]
async fn login_with_wrong_password_is_unauthorized(pool: PgPool) {
    let app = test_app(pool);

    app.clone()
        .oneshot(post_json(
            "/api/auth/register",
            VALID_REGISTER_BODY("wrongpw@example.com"),
        ))
        .await
        .unwrap();

    let response = app
        .oneshot(post_json(
            "/api/auth/login",
            json!({
                "email": "wrongpw@example.com",
                "password": "not the right password",
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn login_with_unknown_email_is_unauthorized(pool: PgPool) {
    let app = test_app(pool);

    let response = app
        .oneshot(post_json(
            "/api/auth/login",
            json!({ "email": "ghost@example.com", "password": "whatever12345" }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn me_without_session_is_unauthorized(pool: PgPool) {
    let app = test_app(pool);

    let response = app.oneshot(get("/api/auth/me", None)).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn me_with_valid_session_returns_the_registered_user(pool: PgPool) {
    let app = test_app(pool);

    let register_response = app
        .clone()
        .oneshot(post_json(
            "/api/auth/register",
            VALID_REGISTER_BODY("me@example.com"),
        ))
        .await
        .unwrap();
    let cookie = session_cookie(&register_response);
    let registered = json_body(register_response).await;

    let response = app
        .oneshot(get("/api/auth/me", Some(&cookie)))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let me = json_body(response).await;
    assert_eq!(me["id"], registered["id"]);
    assert_eq!(me["email"], "me@example.com");
}

#[sqlx::test]
async fn logout_revokes_the_session(pool: PgPool) {
    let app = test_app(pool);

    let register_response = app
        .clone()
        .oneshot(post_json(
            "/api/auth/register",
            VALID_REGISTER_BODY("logout@example.com"),
        ))
        .await
        .unwrap();
    let cookie = session_cookie(&register_response);

    let logout_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);

    let me_response = app
        .oneshot(get("/api/auth/me", Some(&cookie)))
        .await
        .unwrap();
    assert_eq!(me_response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn delete_account_nulls_pii_but_keeps_the_row(pool: PgPool) {
    let app = test_app(pool.clone());

    let register_response = app
        .clone()
        .oneshot(post_json(
            "/api/auth/register",
            VALID_REGISTER_BODY("deleteme@example.com"),
        ))
        .await
        .unwrap();
    let cookie = session_cookie(&register_response);
    let registered = json_body(register_response).await;
    let user_id: uuid::Uuid = serde_json::from_value(registered["id"].clone()).unwrap();

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/auth/account")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // The session was revoked as part of deletion.
    let me_response = app
        .oneshot(get("/api/auth/me", Some(&cookie)))
        .await
        .unwrap();
    assert_eq!(me_response.status(), StatusCode::UNAUTHORIZED);

    // The row itself (and, by extension, anything it references — decks/games) is kept; only
    // PII columns are nulled. This is the GDPR-deletion contract from web/SPEC.md.
    let email: Option<String> = sqlx::query_scalar("select email from users where id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(email, None);
}

#[sqlx::test]
async fn oauth_start_redirects_to_google_and_sets_state_cookie(pool: PgPool) {
    let app = test_app(pool);

    let response = app
        .oneshot(get("/api/auth/oauth/google/start", None))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.starts_with("https://accounts.google.com/"));
    assert!(session_cookie(&response).starts_with("deckgym_oauth_state="));
}

#[sqlx::test]
async fn oauth_start_with_unknown_provider_is_rejected(pool: PgPool) {
    let app = test_app(pool);

    let response = app
        .oneshot(get("/api/auth/oauth/bogus/start", None))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn oauth_callback_without_state_cookie_is_rejected(pool: PgPool) {
    let app = test_app(pool);

    // No prior /start call, so there's no `deckgym_oauth_state` cookie to send — this is what a
    // forged or replayed callback URL looks like, and must not reach Google at all.
    let response = app
        .oneshot(get(
            "/api/auth/oauth/google/callback?code=fake&state=fake",
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn complete_oauth_signup_without_pending_signup_is_rejected(pool: PgPool) {
    let app = test_app(pool);

    let response = app
        .oneshot(post_json(
            "/api/auth/oauth/complete",
            json!({ "training_data_opt_in": true }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn complete_oauth_signup_without_opt_in_is_rejected(pool: PgPool) {
    let app = test_app(pool);

    let response = app
        .oneshot(post_json(
            "/api/auth/oauth/complete",
            json!({ "training_data_opt_in": false }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
