use crate::AppState;
use axum::{extract::State, Json};
use deckgym::{
    card_ids::CardId,
    card_validation::{get_implementation_status, ImplementationStatus},
    database::get_card_by_enum,
    models::Card,
};
use serde::Serialize;
use strum::IntoEnumIterator;

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardType {
    Pokemon,
    Trainer,
}

/// The engine's compiled-in card database has no bulk "all cards" export (only
/// `get_card_by_enum(CardId) -> Card`), so this walks every `CardId` variant instead — the same
/// pattern `src/bin/card_status.rs` uses for the equivalent CLI report.
#[derive(Serialize)]
pub struct CardCatalogEntry {
    /// "SET NUMBER" format, e.g. "A1 001" — matches what `Deck::from_string` parses and what
    /// deckgym.com's builder exports, so deck text built from these ids round-trips as-is.
    id: String,
    name: String,
    card_type: CardType,
    /// Whether this counts toward `Deck::is_valid()`'s "at least 1 Basic" rule — surfaced so
    /// the card picker can flag decks missing one before the server round-trip does.
    is_basic: bool,
    status: ImplementationStatus,
    /// Built from `AppState.config.card_image_base_url` + this card's id (see `card_image_url`
    /// below) — `None` whenever that base URL isn't configured, or for any card whose art
    /// hasn't been uploaded to the host yet. The engine's database doesn't carry image URLs
    /// itself (see `SPEC.md`), so this is assembled per-request rather than stored.
    image_url: Option<String>,
}

/// `id` is "SET NUMBER" (e.g. "A1 001"); the space is swapped for a hyphen since it's easy to
/// mangle by hand in a URL. Mirrored client-side by `cardImageSrc` in
/// `web/frontend/src/lib/gameTypes.ts` for the one screen (the live game board) that renders
/// card art from wasm state directly, without a round trip through this endpoint.
fn card_image_url(base_url: &str, id: &str) -> String {
    format!("{base_url}/{}.png", id.replace(' ', "-"))
}

pub async fn list_cards(State(state): State<AppState>) -> Json<Vec<CardCatalogEntry>> {
    let mut cards: Vec<CardCatalogEntry> = CardId::iter()
        .map(|card_id| {
            let card = get_card_by_enum(card_id);
            let id = card.get_id();
            CardCatalogEntry {
                image_url: state
                    .config
                    .card_image_base_url
                    .as_deref()
                    .map(|base_url| card_image_url(base_url, &id)),
                id,
                name: card.get_name(),
                card_type: match card {
                    Card::Pokemon(_) => CardType::Pokemon,
                    Card::Trainer(_) => CardType::Trainer,
                },
                is_basic: card.is_basic(),
                status: get_implementation_status(card_id),
            }
        })
        .collect();
    cards.sort_by(|a, b| a.id.cmp(&b.id));
    Json(cards)
}
