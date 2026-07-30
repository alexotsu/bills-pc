use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{auth::session::CurrentUser, error::ApiError, models::GameRow, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/games", get(list_games).post(create_game))
        .route("/api/games/:id", get(get_game).patch(update_game_outcome))
        .route(
            "/api/games/:id/plies",
            post(submit_plies).delete(delete_plies_from),
        )
}

const VALID_OUTCOMES: [&str; 3] = ["win", "loss", "tie"];

/// Fetches a game only if it exists *and* is owned by `user_id` — a single query covering both,
/// so "doesn't exist" and "isn't yours" both 404 identically (same reasoning as decks.rs).
async fn find_owned_game(
    pool: &sqlx::PgPool,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<GameRow, ApiError> {
    sqlx::query_as::<_, GameRow>("select * from games where id = $1 and user_id = $2")
        .bind(game_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)
}

#[derive(Deserialize)]
pub struct CreateGameRequest {
    deck_a_id: Uuid,
    deck_b_id: Uuid,
    mode: String,
    /// The wasm engine's seed (a JS `bigint`, so this arrives as a string to avoid precision
    /// loss — `bigint` doesn't round-trip through JSON `number` safely at the high end of i64).
    seed: String,
}

/// Created *before* the first ply arrives — SPEC.md wants even zero-ply abandoned games saved,
/// which only works if the row exists independently of any ply ever being synced.
async fn create_game(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(req): Json<CreateGameRequest>,
) -> Result<Json<GameRow>, ApiError> {
    if req.mode != "hotseat" && req.mode != "ai" {
        return Err(ApiError::BadRequest(
            "mode must be 'hotseat' or 'ai'".to_string(),
        ));
    }
    let seed: i64 = req
        .seed
        .parse()
        .map_err(|_| ApiError::BadRequest("seed must be an integer".to_string()))?;

    for deck_id in [req.deck_a_id, req.deck_b_id] {
        let visible: bool = sqlx::query_scalar(
            "select exists(select 1 from decks where id = $1 and (is_reference = true or user_id = $2))",
        )
        .bind(deck_id)
        .bind(user.id)
        .fetch_one(&state.db)
        .await?;
        if !visible {
            return Err(ApiError::BadRequest(format!(
                "deck {deck_id} not found or not accessible"
            )));
        }
    }

    let game = sqlx::query_as::<_, GameRow>(
        "insert into games (user_id, deck_a_id, deck_b_id, mode, seed) \
         values ($1, $2, $3, $4, $5) returning *",
    )
    .bind(user.id)
    .bind(req.deck_a_id)
    .bind(req.deck_b_id)
    .bind(&req.mode)
    .bind(seed)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(game))
}

#[derive(Deserialize)]
pub struct ListGamesQuery {
    /// `"win"` | `"loss"` | `"tie"` | `"incomplete"` (the last meaning `outcome is null`, not a
    /// stored value — see the `GameRow` doc comment).
    outcome: Option<String>,
    deck_id: Option<Uuid>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct GameListItem {
    id: Uuid,
    deck_a_id: Uuid,
    deck_a_name: String,
    deck_b_id: Uuid,
    deck_b_name: String,
    mode: String,
    outcome: Option<String>,
    seed: i64,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

async fn list_games(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(params): Query<ListGamesQuery>,
) -> Result<Json<Vec<GameListItem>>, ApiError> {
    if let Some(outcome) = &params.outcome {
        if outcome != "incomplete" && !VALID_OUTCOMES.contains(&outcome.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "outcome must be one of: incomplete, {}",
                VALID_OUTCOMES.join(", ")
            )));
        }
    }

    let games = sqlx::query_as::<_, GameListItem>(
        "select g.id, g.deck_a_id, da.name as deck_a_name, g.deck_b_id, db.name as deck_b_name, \
                g.mode, g.outcome, g.seed, g.created_at, g.updated_at \
         from games g \
         join decks da on da.id = g.deck_a_id \
         join decks db on db.id = g.deck_b_id \
         where g.user_id = $1 \
           and ( \
             $2::text is null \
             or ($2 = 'incomplete' and g.outcome is null) \
             or g.outcome = $2 \
           ) \
           and ($3::uuid is null or g.deck_a_id = $3 or g.deck_b_id = $3) \
         order by g.updated_at desc",
    )
    .bind(user.id)
    .bind(&params.outcome)
    .bind(params.deck_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(games))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct GamePlyRow {
    ply: i32,
    actor: i16,
    state: serde_json::Value,
    playable_actions: serde_json::Value,
    chosen_action: serde_json::Value,
}

#[derive(Serialize)]
pub struct GameDetail {
    #[serde(flatten)]
    game: GameRow,
    plies: Vec<GamePlyRow>,
}

/// Full ply history for replay. `state`/`playable_actions`/`chosen_action` are stored and
/// returned as opaque JSON (not deserialized into the engine's own types) deliberately: these
/// rows are a frozen historical record, and coupling them to the engine's *current* struct
/// shapes would break reading older games the moment those shapes change.
async fn get_game(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<GameDetail>, ApiError> {
    let game = find_owned_game(&state.db, id, user.id).await?;
    let plies = sqlx::query_as::<_, GamePlyRow>(
        "select ply, actor, state_json as state, playable_actions_json as playable_actions, \
                chosen_action_json as chosen_action \
         from game_plies where game_id = $1 order by ply asc",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(GameDetail { game, plies }))
}

#[derive(Deserialize)]
pub struct UpdateGameRequest {
    outcome: String,
}

/// Set by both the natural `game_over` path and the manual "Declare Winner" override — either
/// way it's just "this game is now decided, here's how."
async fn update_game_outcome(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateGameRequest>,
) -> Result<Json<GameRow>, ApiError> {
    if !VALID_OUTCOMES.contains(&req.outcome.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "outcome must be one of: {}",
            VALID_OUTCOMES.join(", ")
        )));
    }
    find_owned_game(&state.db, id, user.id).await?;

    let game = sqlx::query_as::<_, GameRow>(
        "update games set outcome = $1, updated_at = now() where id = $2 returning *",
    )
    .bind(&req.outcome)
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(game))
}

#[derive(Deserialize)]
pub struct PlyInput {
    ply: i32,
    actor: i16,
    state: serde_json::Value,
    playable_actions: serde_json::Value,
    chosen_action: serde_json::Value,
}

#[derive(Deserialize)]
pub struct SubmitPliesRequest {
    plies: Vec<PlyInput>,
}

/// `on conflict do nothing` makes this safe to retry with overlapping/already-synced plies —
/// the frontend re-sends its whole unsynced buffer on every attempt rather than tracking
/// per-ply ack state, so retries after a partial failure are expected and must be idempotent.
async fn submit_plies(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<SubmitPliesRequest>,
) -> Result<StatusCode, ApiError> {
    find_owned_game(&state.db, id, user.id).await?;

    let mut tx = state.db.begin().await?;
    for ply in &req.plies {
        sqlx::query(
            "insert into game_plies \
                (game_id, ply, actor, state_json, playable_actions_json, chosen_action_json) \
             values ($1, $2, $3, $4, $5, $6) \
             on conflict (game_id, ply) do nothing",
        )
        .bind(id)
        .bind(ply.ply)
        .bind(ply.actor)
        .bind(&ply.state)
        .bind(&ply.playable_actions)
        .bind(&ply.chosen_action)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("update games set updated_at = now() where id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct DeletePliesQuery {
    from: i32,
}

/// Deletes every ply at or after `from` — used when the frontend's Undo reverts one or more
/// actions, so a decision the player corrected doesn't linger in what's meant to be a clean
/// training-data record. `>=` (not deleting exactly one ply) so repeated undos, and undos of a
/// ply that never actually made it to the backend yet, both converge correctly with a single
/// idempotent call rather than needing to track per-ply sync state.
async fn delete_plies_from(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
    Query(params): Query<DeletePliesQuery>,
) -> Result<StatusCode, ApiError> {
    find_owned_game(&state.db, id, user.id).await?;

    sqlx::query("delete from game_plies where game_id = $1 and ply >= $2")
        .bind(id)
        .bind(params.from)
        .execute(&state.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
