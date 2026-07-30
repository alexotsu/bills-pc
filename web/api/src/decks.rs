use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use deckgym::{card_validation::get_implementation_status, Deck as EngineDeck};
use serde::Deserialize;
use std::collections::BTreeSet;
use uuid::Uuid;

use crate::{
    auth::session::{CurrentUser, OptionalCurrentUser},
    error::ApiError,
    models::DeckRow,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/decks", get(list_decks).post(create_deck))
        .route(
            "/api/decks/:id",
            get(get_deck).put(update_deck).delete(delete_deck),
        )
}

/// Parses and validates `deck_text` the same way the engine itself would use it (20 cards, at
/// least 1 Basic, at most 2 copies of any name), plus rejects any card that isn't fully
/// implemented yet — `SPEC.md`'s "unimplemented cards excluded" rule, enforced here so it can't
/// be bypassed by posting deck text directly rather than going through a card-picker UI.
fn validate_deck_text(deck_text: &str) -> Result<EngineDeck, ApiError> {
    let deck = EngineDeck::from_string(deck_text).map_err(ApiError::BadRequest)?;

    if !deck.is_valid() {
        return Err(ApiError::BadRequest(
            "deck must have exactly 20 cards, at least 1 Basic Pokémon, and at most 2 copies of \
             any card name"
                .to_string(),
        ));
    }

    let unimplemented: BTreeSet<String> = deck
        .cards
        .iter()
        .filter_map(|card| {
            let status = get_implementation_status(card.get_card_id());
            (!status.is_complete())
                .then(|| format!("{} ({})", card.get_name(), status.description()))
        })
        .collect();
    if !unimplemented.is_empty() {
        return Err(ApiError::BadRequest(format!(
            "deck contains unimplemented cards: {}",
            unimplemented.into_iter().collect::<Vec<_>>().join(", ")
        )));
    }

    Ok(deck)
}

fn map_foreign_key_violation(e: sqlx::Error) -> ApiError {
    match &e {
        sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => ApiError::Conflict(
            "deck can't be modified because it's already been used in a game".to_string(),
        ),
        _ => ApiError::Database(e),
    }
}

#[derive(Deserialize)]
pub struct DeckRequest {
    name: String,
    deck_text: String,
}

/// Own decks (if logged in) plus every reference deck (`SPEC.md`'s curated meta-deck list,
/// public regardless of login state) — the deck-builder's "yours" vs. "reference" split reads
/// directly off `is_reference` in the response.
async fn list_decks(
    State(state): State<AppState>,
    OptionalCurrentUser(user): OptionalCurrentUser,
) -> Result<Json<Vec<DeckRow>>, ApiError> {
    let decks = sqlx::query_as::<_, DeckRow>(
        "select * from decks where is_reference = true or user_id = $1 order by created_at desc",
    )
    .bind(user.map(|u| u.id))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(decks))
}

async fn create_deck(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(req): Json<DeckRequest>,
) -> Result<Json<DeckRow>, ApiError> {
    if req.name.trim().is_empty() {
        return Err(ApiError::BadRequest("deck name is required".to_string()));
    }
    validate_deck_text(&req.deck_text)?;

    let deck = sqlx::query_as::<_, DeckRow>(
        "insert into decks (user_id, name, deck_text) values ($1, $2, $3) returning *",
    )
    .bind(user.id)
    .bind(req.name.trim())
    .bind(&req.deck_text)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(deck))
}

/// Visible if it's a reference deck (public) or owned by the caller; 404 either way otherwise
/// (including "exists but isn't yours") so ownership isn't leaked through a 403-vs-404 split.
async fn get_deck(
    State(state): State<AppState>,
    OptionalCurrentUser(user): OptionalCurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<DeckRow>, ApiError> {
    let deck = sqlx::query_as::<_, DeckRow>(
        "select * from decks where id = $1 and (is_reference = true or user_id = $2)",
    )
    .bind(id)
    .bind(user.map(|u| u.id))
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)?;

    Ok(Json(deck))
}

async fn update_deck(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<DeckRequest>,
) -> Result<Json<DeckRow>, ApiError> {
    if req.name.trim().is_empty() {
        return Err(ApiError::BadRequest("deck name is required".to_string()));
    }
    validate_deck_text(&req.deck_text)?;

    // Unlike DELETE, an UPDATE of `name`/`deck_text` doesn't touch the `decks.id` the `games`
    // FK actually points at, so the database won't reject this on its own — the
    // immutability-once-played rule (SPEC.md) has to be checked explicitly here.
    let already_played: bool = sqlx::query_scalar(
        "select exists(select 1 from games where deck_a_id = $1 or deck_b_id = $1)",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    if already_played {
        return Err(ApiError::Conflict(
            "deck can't be edited because it's already been used in a game".to_string(),
        ));
    }

    // Reference decks (user_id is null) never match `user_id = $2` for a real user, so they're
    // 404 here too, not just unwritable — same "don't leak ownership" reasoning as `get_deck`.
    let deck = sqlx::query_as::<_, DeckRow>(
        "update decks set name = $1, deck_text = $2, updated_at = now() \
         where id = $3 and user_id = $4 \
         returning *",
    )
    .bind(req.name.trim())
    .bind(&req.deck_text)
    .bind(id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)?;

    Ok(Json(deck))
}

async fn delete_deck(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let result = sqlx::query("delete from decks where id = $1 and user_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(map_foreign_key_violation)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
