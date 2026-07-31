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
    /// `"win"` | `"loss"` | `"tie"` | `"incomplete"` | `"completed"`. `"incomplete"` means
    /// `outcome is null`; `"completed"` is its complement (`outcome is not null`, i.e. win/loss/
    /// tie collapsed together) — used to back the history view's default "hide incomplete"
    /// filter without the frontend needing to OR together three separate outcome values. Neither
    /// is a stored value — see the `GameRow` doc comment.
    ///
    /// `"win"`/`"loss"` are interpreted **relative to `deck_id` when it's also given** (see
    /// below), not relative to `deck_a`/seat 0 the way the stored column is — a game a deck lost
    /// while playing as deck_b would otherwise never show up under that deck's own "Loss"
    /// filter, since the raw column would say `"win"` (deck_a's result, not this deck's).
    outcome: Option<String>,
    /// When given, also scopes `outcome`'s meaning: `"win"` means *this* deck won, `"loss"`
    /// means it lost, regardless of whether it happened to be `deck_a` or `deck_b` in a given
    /// game. Wins and losses belong to decks, not to the arbitrary "Player 1"/"Player 2" seat
    /// assignment a game happened to start with.
    deck_id: Option<Uuid>,
    /// Narrows further to games played specifically between `deck_id` and this deck (either
    /// order) — backs the head-to-head matchup view. Requires `deck_id` to also be set; on its
    /// own it wouldn't have a first deck to pair against.
    opponent_deck_id: Option<Uuid>,
}

/// `outcome` here is always the raw, stored value — relative to `deck_a` (seat 0), *not*
/// re-oriented around `deck_id` even when one was passed as a filter (unlike the filter itself,
/// see `ListGamesQuery::outcome`). Consumers that need "did this deck win" for an arbitrary deck
/// (including whichever one was filtered on) should compare `outcome` against `deck_a_id`/
/// `deck_b_id` directly, e.g. `outcome == "win" ? deck_a_id : deck_b_id` is the winner — the web
/// frontend's history view does exactly this to label rows by deck name rather than by a
/// player-relative "Win"/"Loss" (see `web/frontend/src/app/games/page.tsx`).
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
        if outcome != "incomplete"
            && outcome != "completed"
            && !VALID_OUTCOMES.contains(&outcome.as_str())
        {
            return Err(ApiError::BadRequest(format!(
                "outcome must be one of: incomplete, completed, {}",
                VALID_OUTCOMES.join(", ")
            )));
        }
    }
    if params.opponent_deck_id.is_some() && params.deck_id.is_none() {
        return Err(ApiError::BadRequest(
            "opponent_deck_id requires deck_id".to_string(),
        ));
    }

    // `deck_relative_outcome` flips win/loss when `deck_id` names `deck_b` for a given row, so
    // the outer WHERE clause's `outcome` filter answers "did the *filtered* deck win/lose", not
    // "did deck_a win/lose" — see the doc comments on `ListGamesQuery::outcome` and
    // `GameListItem`. The final SELECT still returns the raw, deck_a-relative `outcome` column
    // unchanged; only the filtering semantics are deck-relative here.
    //
    // The deck-matching clause has two modes: with just `deck_id`, a game matches if that deck
    // played either side (the existing "games involving this deck" filter); with
    // `opponent_deck_id` too, it narrows to games between *exactly* that pair, either order —
    // the head-to-head matchup filter.
    let games = sqlx::query_as::<_, GameListItem>(
        "with scoped as ( \
           select g.id, g.deck_a_id, da.name as deck_a_name, g.deck_b_id, db.name as deck_b_name, \
                  g.mode, g.outcome, g.seed, g.created_at, g.updated_at, \
                  case \
                    when $3::uuid is not null and g.deck_b_id = $3 then \
                      case g.outcome when 'win' then 'loss' when 'loss' then 'win' else g.outcome end \
                    else g.outcome \
                  end as deck_relative_outcome \
           from games g \
           join decks da on da.id = g.deck_a_id \
           join decks db on db.id = g.deck_b_id \
           where g.user_id = $1 \
             and ( \
               ($4::uuid is null and ($3::uuid is null or g.deck_a_id = $3 or g.deck_b_id = $3)) \
               or ($4::uuid is not null and ( \
                 (g.deck_a_id = $3 and g.deck_b_id = $4) or (g.deck_a_id = $4 and g.deck_b_id = $3) \
               )) \
             ) \
         ) \
         select id, deck_a_id, deck_a_name, deck_b_id, deck_b_name, mode, outcome, seed, \
                created_at, updated_at \
         from scoped \
         where ( \
           $2::text is null \
           or ($2 = 'incomplete' and deck_relative_outcome is null) \
           or ($2 = 'completed' and deck_relative_outcome is not null) \
           or deck_relative_outcome = $2 \
         ) \
         order by updated_at desc",
    )
    .bind(user.id)
    .bind(&params.outcome)
    .bind(params.deck_id)
    .bind(params.opponent_deck_id)
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

/// Like `GameRow` but with deck names joined in, so the replay view can label the outcome by
/// deck name (e.g. "Suicune Baxcalibur won") instead of a player-relative "Win"/"Loss" — same
/// reasoning as `GameListItem`.
#[derive(Serialize, sqlx::FromRow)]
pub struct GameWithDeckNames {
    id: Uuid,
    user_id: Option<Uuid>,
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

#[derive(Serialize)]
pub struct GameDetail {
    #[serde(flatten)]
    game: GameWithDeckNames,
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
    let game = sqlx::query_as::<_, GameWithDeckNames>(
        "select g.id, g.user_id, g.deck_a_id, da.name as deck_a_name, g.deck_b_id, \
                db.name as deck_b_name, g.mode, g.outcome, g.seed, g.created_at, g.updated_at \
         from games g \
         join decks da on da.id = g.deck_a_id \
         join decks db on db.id = g.deck_b_id \
         where g.id = $1 and g.user_id = $2",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)?;

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
