use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: Option<String>,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub oauth_provider: Option<String>,
    #[serde(skip_serializing)]
    pub oauth_subject: Option<String>,
    pub training_data_opt_in: bool,
    pub created_at: DateTime<Utc>,
}

/// A `decks` row. Named `DeckRow` (not `Deck`) to stay unambiguous alongside
/// `deckgym::Deck` — the engine's parsed, in-memory representation of `deck_text` — which the
/// deck-CRUD handlers need in the same scope for validation.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct DeckRow {
    pub id: Uuid,
    /// `None` for reference decks (`SPEC.md`'s curated meta-deck list) — those are
    /// admin-seeded and have no owner.
    pub user_id: Option<Uuid>,
    pub name: String,
    pub deck_text: String,
    pub is_reference: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A `games` row. `outcome` is `None` while a game is still in progress (or was abandoned
/// without ever being marked finished — "incomplete" for filtering purposes is just this, not a
/// distinct stored value; see `games::list_games`). Recorded relative to `deck_a`/seat 0: `"win"`
/// means seat 0 won, `"loss"` means seat 1 won, matching how a deck-testing tool naturally wants
/// to ask "how did *this* deck do" with one fixed reference deck per game.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct GameRow {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub deck_a_id: Uuid,
    pub deck_b_id: Uuid,
    pub mode: String,
    pub outcome: Option<String>,
    pub seed: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
