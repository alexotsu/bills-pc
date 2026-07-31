//! Integration tests for deck CRUD, driven through the real Axum router (`api::app`) against a
//! throwaway Postgres database per test (`#[sqlx::test]`) — same approach as `tests/auth.rs`.

use api::{app, config::Config, config::OAuthConfig, AppState};
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

// A real, small, fully-implemented deck (same one proven end-to-end in
// web/frontend/src/app/scaffold-check/page.tsx) — used wherever a test needs deck text that
// should pass validation.
const VALID_DECK_TEXT: &str = r#"Pokémon: 10
2 Bulbasaur A1 001
2 Exeggcute A1 021
2 Exeggutor ex A1 023
2 Ivysaur A1 002
2 Venusaur ex A1 004

Trainer: 10
2 Professor's Research P-A 007
2 Poké Ball P-A 005
2 Erika A1 219
1 Sabrina A1 225
2 X Speed P-A 002
1 Red Card P-A 006
"#;

fn test_config() -> Config {
    Config {
        database_url: String::new(),
        oauth: OAuthConfig {
            google_client_id: "test-google-client-id".to_string(),
            google_client_secret: "test-google-client-secret".to_string(),
            facebook_client_id: String::new(),
            facebook_client_secret: String::new(),
            api_base_url: "http://localhost:8080".to_string(),
        },
        frontend_url: "http://localhost:3000".to_string(),
        cookie_secure: false,
        card_image_base_url: None,
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
    request("POST", uri, None, Some(body))
}

fn get(uri: &str, cookie: Option<&str>) -> Request<Body> {
    request("GET", uri, cookie, None)
}

fn put_json(uri: &str, cookie: &str, body: Value) -> Request<Body> {
    request("PUT", uri, Some(cookie), Some(body))
}

fn delete(uri: &str, cookie: &str) -> Request<Body> {
    request("DELETE", uri, Some(cookie), None)
}

fn request(method: &str, uri: &str, cookie: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    match body {
        Some(body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

/// Registers a fresh user and returns (session cookie, user id) — every deck route needs a
/// logged-in caller to set up, so this is the shared arrange-step for most tests here.
async fn register(app: &axum::Router, email: &str) -> (String, Uuid) {
    let response = app
        .clone()
        .oneshot(post_json(
            "/api/auth/register",
            json!({
                "email": email,
                "password": "correct horse battery staple",
                "training_data_opt_in": true,
            }),
        ))
        .await
        .unwrap();
    let cookie = session_cookie(&response);
    let user: Value = json_body(response).await;
    let user_id: Uuid = serde_json::from_value(user["id"].clone()).unwrap();
    (cookie, user_id)
}

async fn insert_reference_deck(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar(
        "insert into decks (user_id, name, deck_text, is_reference) \
         values (null, $1, $2, true) returning id",
    )
    .bind(name)
    .bind(VALID_DECK_TEXT)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Simulates "this deck has been played" without needing the (not-yet-built) games API —
/// inserts a minimal `games` row referencing it directly.
async fn mark_deck_as_played(pool: &PgPool, user_id: Uuid, deck_id: Uuid) {
    sqlx::query(
        "insert into games (user_id, deck_a_id, deck_b_id, mode, seed) \
         values ($1, $2, $2, 'hotseat', 0)",
    )
    .bind(user_id)
    .bind(deck_id)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn create_deck_persists_and_returns_it(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, user_id) = register(&app, "creator@example.com").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/decks")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    json!({ "name": "My Deck", "deck_text": VALID_DECK_TEXT }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let deck = json_body(response).await;
    assert_eq!(deck["name"], "My Deck");
    assert_eq!(deck["is_reference"], false);
    assert_eq!(deck["user_id"], user_id.to_string());
}

#[sqlx::test]
async fn create_deck_without_login_is_unauthorized(pool: PgPool) {
    let app = test_app(pool);

    let response = app
        .oneshot(post_json(
            "/api/decks",
            json!({ "name": "Nope", "deck_text": VALID_DECK_TEXT }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn create_deck_with_empty_name_is_rejected(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "emptyname@example.com").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/decks")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    json!({ "name": "   ", "deck_text": VALID_DECK_TEXT }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn create_deck_with_garbage_deck_text_is_rejected(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "garbage@example.com").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/decks")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    json!({ "name": "Garbage", "deck_text": "not a real deck" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn create_deck_with_wrong_card_count_is_rejected(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "wrongcount@example.com").await;

    // Same as VALID_DECK_TEXT but missing the last card, so it's 19 cards, not 20 —
    // Deck::is_valid() should reject this.
    let nineteen_cards = VALID_DECK_TEXT.replace("1 Red Card P-A 006\n", "");

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/decks")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    json!({ "name": "Nineteen", "deck_text": nineteen_cards }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn list_decks_mixes_own_decks_with_reference_decks_only(pool: PgPool) {
    let app = test_app(pool.clone());
    let (cookie_a, _) = register(&app, "owner-a@example.com").await;
    let (cookie_b, _) = register(&app, "owner-b@example.com").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/decks")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie_a)
                .body(Body::from(
                    json!({ "name": "A's Deck", "deck_text": VALID_DECK_TEXT }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let a_deck_id = json_body(create_response).await["id"].clone();

    insert_reference_deck(&pool, "Reference Deck").await;

    // Owner A sees their own deck plus the reference deck.
    let as_owner_a = json_body(
        app.clone()
            .oneshot(get("/api/decks", Some(&cookie_a)))
            .await
            .unwrap(),
    )
    .await;
    let names_a: Vec<String> = as_owner_a
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap().to_string())
        .collect();
    assert!(names_a.contains(&"A's Deck".to_string()));
    assert!(names_a.contains(&"Reference Deck".to_string()));

    // Owner B sees only the reference deck, not A's.
    let as_owner_b = json_body(
        app.clone()
            .oneshot(get("/api/decks", Some(&cookie_b)))
            .await
            .unwrap(),
    )
    .await;
    let names_b: Vec<String> = as_owner_b
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap().to_string())
        .collect();
    assert!(!names_b.contains(&"A's Deck".to_string()));
    assert!(names_b.contains(&"Reference Deck".to_string()));

    // Anonymous (no cookie) also sees only the reference deck.
    let as_anonymous = json_body(app.oneshot(get("/api/decks", None)).await.unwrap()).await;
    let names_anon: Vec<String> = as_anonymous
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap().to_string())
        .collect();
    assert!(!names_anon.contains(&"A's Deck".to_string()));
    assert!(names_anon.contains(&"Reference Deck".to_string()));
    let _ = a_deck_id;
}

#[sqlx::test]
async fn get_deck_not_visible_to_other_user_is_404(pool: PgPool) {
    let app = test_app(pool);
    let (cookie_a, _) = register(&app, "getowner@example.com").await;
    let (cookie_b, _) = register(&app, "getother@example.com").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/decks")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie_a)
                .body(Body::from(
                    json!({ "name": "Private", "deck_text": VALID_DECK_TEXT }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let deck_id = json_body(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let owner_response = app
        .clone()
        .oneshot(get(&format!("/api/decks/{deck_id}"), Some(&cookie_a)))
        .await
        .unwrap();
    assert_eq!(owner_response.status(), StatusCode::OK);

    let other_response = app
        .oneshot(get(&format!("/api/decks/{deck_id}"), Some(&cookie_b)))
        .await
        .unwrap();
    assert_eq!(other_response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn get_reference_deck_is_visible_without_login(pool: PgPool) {
    let app = test_app(pool.clone());
    let deck_id = insert_reference_deck(&pool, "Public Reference").await;

    let response = app
        .oneshot(get(&format!("/api/decks/{deck_id}"), None))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let deck = json_body(response).await;
    assert_eq!(deck["is_reference"], true);
}

#[sqlx::test]
async fn update_deck_changes_name_and_text(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "editor@example.com").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/decks")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    json!({ "name": "Before", "deck_text": VALID_DECK_TEXT }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let deck_id = json_body(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let response = app
        .oneshot(put_json(
            &format!("/api/decks/{deck_id}"),
            &cookie,
            json!({ "name": "After", "deck_text": VALID_DECK_TEXT }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let deck = json_body(response).await;
    assert_eq!(deck["name"], "After");
}

#[sqlx::test]
async fn update_deck_by_non_owner_is_404(pool: PgPool) {
    let app = test_app(pool);
    let (cookie_a, _) = register(&app, "putowner@example.com").await;
    let (cookie_b, _) = register(&app, "putother@example.com").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/decks")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie_a)
                .body(Body::from(
                    json!({ "name": "Mine", "deck_text": VALID_DECK_TEXT }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let deck_id = json_body(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let response = app
        .oneshot(put_json(
            &format!("/api/decks/{deck_id}"),
            &cookie_b,
            json!({ "name": "Hijacked", "deck_text": VALID_DECK_TEXT }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn update_deck_already_used_in_a_game_is_rejected(pool: PgPool) {
    let app = test_app(pool.clone());
    let (cookie, user_id) = register(&app, "playedputter@example.com").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/decks")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    json!({ "name": "Played", "deck_text": VALID_DECK_TEXT }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let deck_id: Uuid =
        serde_json::from_value(json_body(create_response).await["id"].clone()).unwrap();

    mark_deck_as_played(&pool, user_id, deck_id).await;

    let response = app
        .oneshot(put_json(
            &format!("/api/decks/{deck_id}"),
            &cookie,
            json!({ "name": "Changed after playing", "deck_text": VALID_DECK_TEXT }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[sqlx::test]
async fn delete_deck_removes_it(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "deleter@example.com").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/decks")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    json!({ "name": "Doomed", "deck_text": VALID_DECK_TEXT }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let deck_id = json_body(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let delete_response = app
        .clone()
        .oneshot(delete(&format!("/api/decks/{deck_id}"), &cookie))
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let get_response = app
        .oneshot(get(&format!("/api/decks/{deck_id}"), Some(&cookie)))
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn delete_deck_by_non_owner_is_404(pool: PgPool) {
    let app = test_app(pool);
    let (cookie_a, _) = register(&app, "deleteowner@example.com").await;
    let (cookie_b, _) = register(&app, "deleteother@example.com").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/decks")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie_a)
                .body(Body::from(
                    json!({ "name": "NotYours", "deck_text": VALID_DECK_TEXT }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let deck_id = json_body(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let response = app
        .oneshot(delete(&format!("/api/decks/{deck_id}"), &cookie_b))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn delete_deck_already_used_in_a_game_is_rejected(pool: PgPool) {
    let app = test_app(pool.clone());
    let (cookie, user_id) = register(&app, "playeddeleter@example.com").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/decks")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    json!({ "name": "Played", "deck_text": VALID_DECK_TEXT }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let deck_id: Uuid =
        serde_json::from_value(json_body(create_response).await["id"].clone()).unwrap();

    mark_deck_as_played(&pool, user_id, deck_id).await;

    let response = app
        .oneshot(delete(&format!("/api/decks/{deck_id}"), &cookie))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}
