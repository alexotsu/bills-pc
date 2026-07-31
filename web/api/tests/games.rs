//! Integration tests for battle-history persistence (games + plies), driven through the real
//! Axum router (`api::app`) against a throwaway Postgres database per test — same approach as
//! `tests/auth.rs`/`tests/decks.rs`.

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

fn post_json(uri: &str, cookie: &str, body: Value) -> Request<Body> {
    request("POST", uri, Some(cookie), Some(body))
}

fn patch_json(uri: &str, cookie: &str, body: Value) -> Request<Body> {
    request("PATCH", uri, Some(cookie), Some(body))
}

fn get(uri: &str, cookie: &str) -> Request<Body> {
    request("GET", uri, Some(cookie), None)
}

async fn register(app: &axum::Router, email: &str) -> (String, Uuid) {
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({
                "email": email,
                "password": "correct horse battery staple",
                "training_data_opt_in": true,
            })),
        ))
        .await
        .unwrap();
    let cookie = session_cookie(&response);
    let user: Value = json_body(response).await;
    let user_id: Uuid = serde_json::from_value(user["id"].clone()).unwrap();
    (cookie, user_id)
}

async fn create_deck(app: &axum::Router, cookie: &str, name: &str) -> Uuid {
    let response = app
        .clone()
        .oneshot(post_json(
            "/api/decks",
            cookie,
            json!({ "name": name, "deck_text": VALID_DECK_TEXT }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "deck creation failed");
    let deck = json_body(response).await;
    serde_json::from_value(deck["id"].clone()).unwrap()
}

async fn create_game(app: &axum::Router, cookie: &str, deck_a: Uuid, deck_b: Uuid) -> Value {
    let response = app
        .clone()
        .oneshot(post_json(
            "/api/games",
            cookie,
            json!({
                "deck_a_id": deck_a,
                "deck_b_id": deck_b,
                "mode": "hotseat",
                "seed": "42",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "game creation failed");
    json_body(response).await
}

fn sample_ply(ply: i64) -> Value {
    json!({
        "ply": ply,
        "actor": 0,
        "state": {"turn_count": ply, "fake": true},
        "playable_actions": [{"actor": 0, "action": "EndTurn", "is_stack": false}],
        "chosen_action": {"actor": 0, "action": "EndTurn", "is_stack": false},
    })
}

#[sqlx::test]
async fn create_game_persists_and_returns_it(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, user_id) = register(&app, "creator@example.com").await;
    let deck_a = create_deck(&app, &cookie, "Deck A").await;
    let deck_b = create_deck(&app, &cookie, "Deck B").await;

    let game = create_game(&app, &cookie, deck_a, deck_b).await;
    assert_eq!(game["deck_a_id"], deck_a.to_string());
    assert_eq!(game["deck_b_id"], deck_b.to_string());
    assert_eq!(game["mode"], "hotseat");
    assert_eq!(game["user_id"], user_id.to_string());
    assert!(game["outcome"].is_null());
}

#[sqlx::test]
async fn get_game_includes_deck_names(pool: PgPool) {
    // Deck names let the frontend label the outcome by deck ("Deck A won") instead of a
    // player-relative "Win"/"Loss" — get_game needs to join them in just like list_games does.
    let app = test_app(pool);
    let (cookie, _) = register(&app, "detailnames@example.com").await;
    let deck_a = create_deck(&app, &cookie, "Deck A").await;
    let deck_b = create_deck(&app, &cookie, "Deck B").await;
    let game = create_game(&app, &cookie, deck_a, deck_b).await;
    let game_id: Uuid = serde_json::from_value(game["id"].clone()).unwrap();

    let detail = json_body(
        app.oneshot(get(&format!("/api/games/{game_id}"), &cookie))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(detail["deck_a_name"], "Deck A");
    assert_eq!(detail["deck_b_name"], "Deck B");
}

#[sqlx::test]
async fn create_game_without_login_is_unauthorized(pool: PgPool) {
    let app = test_app(pool);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/games")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "deck_a_id": Uuid::nil(),
                        "deck_b_id": Uuid::nil(),
                        "mode": "hotseat",
                        "seed": "1",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn create_game_with_invalid_mode_is_rejected(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "badmode@example.com").await;
    let deck_a = create_deck(&app, &cookie, "Deck A").await;
    let deck_b = create_deck(&app, &cookie, "Deck B").await;

    let response = app
        .oneshot(post_json(
            "/api/games",
            &cookie,
            json!({
                "deck_a_id": deck_a,
                "deck_b_id": deck_b,
                "mode": "ranked",
                "seed": "1",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn create_game_with_inaccessible_deck_is_rejected(pool: PgPool) {
    let app = test_app(pool);
    let (cookie_a, _) = register(&app, "owner@example.com").await;
    let (cookie_b, _) = register(&app, "other@example.com").await;
    let deck_a = create_deck(&app, &cookie_a, "Owner's Deck").await;
    let not_visible_deck = create_deck(&app, &cookie_b, "Other's Deck").await;

    let response = app
        .oneshot(post_json(
            "/api/games",
            &cookie_a,
            json!({
                "deck_a_id": deck_a,
                "deck_b_id": not_visible_deck,
                "mode": "hotseat",
                "seed": "1",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn submit_plies_persists_and_deduplicates_on_retry(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "plies@example.com").await;
    let deck_a = create_deck(&app, &cookie, "Deck A").await;
    let deck_b = create_deck(&app, &cookie, "Deck B").await;
    let game = create_game(&app, &cookie, deck_a, deck_b).await;
    let game_id: Uuid = serde_json::from_value(game["id"].clone()).unwrap();

    let plies_body = json!({ "plies": [sample_ply(0), sample_ply(1)] });

    // Submit the same batch twice, simulating a retried sync after a dropped response.
    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(post_json(
                &format!("/api/games/{game_id}/plies"),
                &cookie,
                plies_body.clone(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    let detail = json_body(
        app.oneshot(get(&format!("/api/games/{game_id}"), &cookie))
            .await
            .unwrap(),
    )
    .await;
    let plies = detail["plies"].as_array().unwrap();
    assert_eq!(
        plies.len(),
        2,
        "duplicate submission must not double-insert"
    );
    assert_eq!(plies[0]["ply"], 0);
    assert_eq!(plies[1]["ply"], 1);
}

#[sqlx::test]
async fn submit_plies_for_unowned_game_is_404(pool: PgPool) {
    let app = test_app(pool);
    let (cookie_a, _) = register(&app, "plyowner@example.com").await;
    let (cookie_b, _) = register(&app, "plyother@example.com").await;
    let deck_a = create_deck(&app, &cookie_a, "Deck A").await;
    let deck_b = create_deck(&app, &cookie_a, "Deck B").await;
    let game = create_game(&app, &cookie_a, deck_a, deck_b).await;
    let game_id: Uuid = serde_json::from_value(game["id"].clone()).unwrap();

    let response = app
        .oneshot(post_json(
            &format!("/api/games/{game_id}/plies"),
            &cookie_b,
            json!({ "plies": [sample_ply(0)] }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn delete_plies_from_removes_trailing_plies_for_undo(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "undoplies@example.com").await;
    let deck_a = create_deck(&app, &cookie, "Deck A").await;
    let deck_b = create_deck(&app, &cookie, "Deck B").await;
    let game = create_game(&app, &cookie, deck_a, deck_b).await;
    let game_id: Uuid = serde_json::from_value(game["id"].clone()).unwrap();

    app.clone()
        .oneshot(post_json(
            &format!("/api/games/{game_id}/plies"),
            &cookie,
            json!({ "plies": [sample_ply(0), sample_ply(1), sample_ply(2)] }),
        ))
        .await
        .unwrap();

    // Undoing the last action reverts ply 2 (and anything after it, though there's nothing
    // here) — mirrors the frontend calling this with `from` set to the popped ply's number.
    let response = app
        .clone()
        .oneshot(request(
            "DELETE",
            &format!("/api/games/{game_id}/plies?from=2"),
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let detail = json_body(
        app.oneshot(get(&format!("/api/games/{game_id}"), &cookie))
            .await
            .unwrap(),
    )
    .await;
    let plies = detail["plies"].as_array().unwrap();
    assert_eq!(plies.len(), 2, "ply 2 should have been removed");
    assert_eq!(plies[0]["ply"], 0);
    assert_eq!(plies[1]["ply"], 1);
}

#[sqlx::test]
async fn delete_plies_from_for_unowned_game_is_404(pool: PgPool) {
    let app = test_app(pool);
    let (cookie_a, _) = register(&app, "undoowner@example.com").await;
    let (cookie_b, _) = register(&app, "undoother@example.com").await;
    let deck_a = create_deck(&app, &cookie_a, "Deck A").await;
    let deck_b = create_deck(&app, &cookie_a, "Deck B").await;
    let game = create_game(&app, &cookie_a, deck_a, deck_b).await;
    let game_id: Uuid = serde_json::from_value(game["id"].clone()).unwrap();

    let response = app
        .oneshot(request(
            "DELETE",
            &format!("/api/games/{game_id}/plies?from=0"),
            Some(&cookie_b),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn update_game_outcome_sets_it(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "outcome@example.com").await;
    let deck_a = create_deck(&app, &cookie, "Deck A").await;
    let deck_b = create_deck(&app, &cookie, "Deck B").await;
    let game = create_game(&app, &cookie, deck_a, deck_b).await;
    let game_id: Uuid = serde_json::from_value(game["id"].clone()).unwrap();

    let response = app
        .oneshot(patch_json(
            &format!("/api/games/{game_id}"),
            &cookie,
            json!({ "outcome": "win" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let updated = json_body(response).await;
    assert_eq!(updated["outcome"], "win");
}

#[sqlx::test]
async fn update_game_outcome_rejects_invalid_value(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "badoutcome@example.com").await;
    let deck_a = create_deck(&app, &cookie, "Deck A").await;
    let deck_b = create_deck(&app, &cookie, "Deck B").await;
    let game = create_game(&app, &cookie, deck_a, deck_b).await;
    let game_id: Uuid = serde_json::from_value(game["id"].clone()).unwrap();

    let response = app
        .oneshot(patch_json(
            &format!("/api/games/{game_id}"),
            &cookie,
            json!({ "outcome": "victory" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn list_games_filters_by_outcome_and_incomplete(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "filter@example.com").await;
    let deck_a = create_deck(&app, &cookie, "Deck A").await;
    let deck_b = create_deck(&app, &cookie, "Deck B").await;

    let won_game = create_game(&app, &cookie, deck_a, deck_b).await;
    let won_id: Uuid = serde_json::from_value(won_game["id"].clone()).unwrap();
    app.clone()
        .oneshot(patch_json(
            &format!("/api/games/{won_id}"),
            &cookie,
            json!({ "outcome": "win" }),
        ))
        .await
        .unwrap();

    let unfinished_game = create_game(&app, &cookie, deck_a, deck_b).await;
    let unfinished_id: Uuid = serde_json::from_value(unfinished_game["id"].clone()).unwrap();

    let win_only = json_body(
        app.clone()
            .oneshot(get("/api/games?outcome=win", &cookie))
            .await
            .unwrap(),
    )
    .await;
    let win_ids: Vec<Uuid> = win_only
        .as_array()
        .unwrap()
        .iter()
        .map(|g| serde_json::from_value(g["id"].clone()).unwrap())
        .collect();
    assert_eq!(win_ids, vec![won_id]);

    let incomplete_only = json_body(
        app.clone()
            .oneshot(get("/api/games?outcome=incomplete", &cookie))
            .await
            .unwrap(),
    )
    .await;
    let incomplete_ids: Vec<Uuid> = incomplete_only
        .as_array()
        .unwrap()
        .iter()
        .map(|g| serde_json::from_value(g["id"].clone()).unwrap())
        .collect();
    assert_eq!(incomplete_ids, vec![unfinished_id]);

    // "completed" is incomplete's complement (outcome is not null) — backs the history view's
    // default "hide incomplete" filter.
    let completed_only = json_body(
        app.oneshot(get("/api/games?outcome=completed", &cookie))
            .await
            .unwrap(),
    )
    .await;
    let completed_ids: Vec<Uuid> = completed_only
        .as_array()
        .unwrap()
        .iter()
        .map(|g| serde_json::from_value(g["id"].clone()).unwrap())
        .collect();
    assert_eq!(completed_ids, vec![won_id]);
}

#[sqlx::test]
async fn list_games_outcome_filter_is_relative_to_the_filtered_deck(pool: PgPool) {
    // deck_a beat deck_b (stored outcome is "win", relative to deck_a/seat 0). From deck_b's own
    // perspective that's a loss — filtering by deck_b should surface it under "loss", not "win",
    // even though the raw column never changes. A win for one deck is a loss for the other; it
    // shouldn't matter which one happened to be "Player 1" that game.
    let app = test_app(pool);
    let (cookie, _) = register(&app, "deckrelative@example.com").await;
    let deck_a = create_deck(&app, &cookie, "Winner Deck").await;
    let deck_b = create_deck(&app, &cookie, "Loser Deck").await;

    let game = create_game(&app, &cookie, deck_a, deck_b).await;
    let game_id: Uuid = serde_json::from_value(game["id"].clone()).unwrap();
    app.clone()
        .oneshot(patch_json(
            &format!("/api/games/{game_id}"),
            &cookie,
            json!({ "outcome": "win" }),
        ))
        .await
        .unwrap();

    let fetch_ids = |app: axum::Router, uri: String, cookie: String| async move {
        let body = json_body(app.oneshot(get(&uri, &cookie)).await.unwrap()).await;
        body.as_array()
            .unwrap()
            .iter()
            .map(|g| serde_json::from_value::<Uuid>(g["id"].clone()).unwrap())
            .collect::<Vec<_>>()
    };

    // deck_a ("Winner Deck") really did win.
    assert_eq!(
        fetch_ids(
            app.clone(),
            format!("/api/games?outcome=win&deck_id={deck_a}"),
            cookie.clone()
        )
        .await,
        vec![game_id]
    );
    assert_eq!(
        fetch_ids(
            app.clone(),
            format!("/api/games?outcome=loss&deck_id={deck_a}"),
            cookie.clone()
        )
        .await,
        Vec::<Uuid>::new()
    );

    // deck_b ("Loser Deck") lost — flipped relative to the raw, deck_a-relative stored value.
    assert_eq!(
        fetch_ids(
            app.clone(),
            format!("/api/games?outcome=loss&deck_id={deck_b}"),
            cookie.clone()
        )
        .await,
        vec![game_id]
    );
    assert_eq!(
        fetch_ids(
            app.clone(),
            format!("/api/games?outcome=win&deck_id={deck_b}"),
            cookie.clone()
        )
        .await,
        Vec::<Uuid>::new()
    );

    // Regardless of which deck was filtered on, the returned `outcome` field itself stays raw
    // (relative to deck_a) — see the doc comment on `GameListItem`.
    let filtered_by_b = json_body(
        app.oneshot(get(
            &format!("/api/games?outcome=loss&deck_id={deck_b}"),
            &cookie,
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(filtered_by_b[0]["outcome"], "win");
}

#[sqlx::test]
async fn list_games_opponent_deck_id_narrows_to_the_exact_matchup(pool: PgPool) {
    // Backs the head-to-head matchup view: given two decks, only games played specifically
    // between *that pair* (either seat order), not every game either deck happened to be in.
    let app = test_app(pool);
    let (cookie, _) = register(&app, "matchup@example.com").await;
    let deck_a = create_deck(&app, &cookie, "Deck A").await;
    let deck_b = create_deck(&app, &cookie, "Deck B").await;
    let deck_c = create_deck(&app, &cookie, "Deck C").await;

    // A vs B, with A in deck_a's seat.
    let game_a_vs_b = create_game(&app, &cookie, deck_a, deck_b).await;
    let game_a_vs_b_id: Uuid = serde_json::from_value(game_a_vs_b["id"].clone()).unwrap();
    // A vs B again, seats flipped (B in deck_a's seat this time).
    let game_b_vs_a = create_game(&app, &cookie, deck_b, deck_a).await;
    let game_b_vs_a_id: Uuid = serde_json::from_value(game_b_vs_a["id"].clone()).unwrap();
    // A vs C — same deck_a (A) as the first game, but not the A-vs-B matchup.
    create_game(&app, &cookie, deck_a, deck_c).await;

    let fetch_ids = |app: axum::Router, uri: String, cookie: String| async move {
        let body = json_body(app.oneshot(get(&uri, &cookie)).await.unwrap()).await;
        let mut ids: Vec<Uuid> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|g| serde_json::from_value(g["id"].clone()).unwrap())
            .collect();
        ids.sort();
        ids
    };

    let mut expected = vec![game_a_vs_b_id, game_b_vs_a_id];
    expected.sort();

    // Order-agnostic: filtering by (A, B) or (B, A) returns the same two games.
    assert_eq!(
        fetch_ids(
            app.clone(),
            format!("/api/games?deck_id={deck_a}&opponent_deck_id={deck_b}"),
            cookie.clone()
        )
        .await,
        expected
    );
    assert_eq!(
        fetch_ids(
            app.clone(),
            format!("/api/games?deck_id={deck_b}&opponent_deck_id={deck_a}"),
            cookie.clone()
        )
        .await,
        expected
    );

    // opponent_deck_id without deck_id doesn't have a first deck to pair against.
    let response = app
        .oneshot(get(
            &format!("/api/games?opponent_deck_id={deck_b}"),
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn list_games_filters_by_deck(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "deckfilter@example.com").await;
    let deck_a = create_deck(&app, &cookie, "Deck A").await;
    let deck_b = create_deck(&app, &cookie, "Deck B").await;
    let deck_c = create_deck(&app, &cookie, "Deck C").await;

    let game_ab = create_game(&app, &cookie, deck_a, deck_b).await;
    let game_ab_id: Uuid = serde_json::from_value(game_ab["id"].clone()).unwrap();
    create_game(&app, &cookie, deck_b, deck_c).await;

    let filtered = json_body(
        app.oneshot(get(&format!("/api/games?deck_id={deck_a}"), &cookie))
            .await
            .unwrap(),
    )
    .await;
    let ids: Vec<Uuid> = filtered
        .as_array()
        .unwrap()
        .iter()
        .map(|g| serde_json::from_value(g["id"].clone()).unwrap())
        .collect();
    assert_eq!(ids, vec![game_ab_id]);
}

#[sqlx::test]
async fn list_games_only_shows_the_caller_own_games(pool: PgPool) {
    let app = test_app(pool);
    let (cookie_a, _) = register(&app, "listowner@example.com").await;
    let (cookie_b, _) = register(&app, "listother@example.com").await;
    let deck_a = create_deck(&app, &cookie_a, "Deck A").await;
    let deck_b = create_deck(&app, &cookie_a, "Deck B").await;
    create_game(&app, &cookie_a, deck_a, deck_b).await;

    let as_other = json_body(app.oneshot(get("/api/games", &cookie_b)).await.unwrap()).await;
    assert_eq!(as_other.as_array().unwrap().len(), 0);
}

#[sqlx::test]
async fn get_game_not_owned_is_404(pool: PgPool) {
    let app = test_app(pool);
    let (cookie_a, _) = register(&app, "getowner@example.com").await;
    let (cookie_b, _) = register(&app, "getother@example.com").await;
    let deck_a = create_deck(&app, &cookie_a, "Deck A").await;
    let deck_b = create_deck(&app, &cookie_a, "Deck B").await;
    let game = create_game(&app, &cookie_a, deck_a, deck_b).await;
    let game_id: Uuid = serde_json::from_value(game["id"].clone()).unwrap();

    let response = app
        .oneshot(get(&format!("/api/games/{game_id}"), &cookie_b))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// End-to-end confirmation that Phase 2's "can't edit/delete a deck already used in a game"
/// guard (previously untestable — the games table was never actually populated before this
/// phase) is now really wired up against real game rows.
#[sqlx::test]
async fn deck_used_in_a_game_cannot_be_deleted(pool: PgPool) {
    let app = test_app(pool);
    let (cookie, _) = register(&app, "playeddeck@example.com").await;
    let deck_a = create_deck(&app, &cookie, "Deck A").await;
    let deck_b = create_deck(&app, &cookie, "Deck B").await;
    create_game(&app, &cookie, deck_a, deck_b).await;

    let response = app
        .oneshot(request(
            "DELETE",
            &format!("/api/decks/{deck_a}"),
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}
