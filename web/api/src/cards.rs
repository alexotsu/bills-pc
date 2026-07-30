use axum::Json;
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
    /// Not sourced yet (`SPEC.md` calls for card images; the engine's database doesn't carry
    /// image URLs) — placeholder so the frontend's card picker can already shape itself around
    /// this field.
    image_url: Option<String>,
}

pub async fn list_cards() -> Json<Vec<CardCatalogEntry>> {
    let mut cards: Vec<CardCatalogEntry> = CardId::iter()
        .map(|card_id| {
            let card = get_card_by_enum(card_id);
            CardCatalogEntry {
                id: card.get_id(),
                name: card.get_name(),
                card_type: match card {
                    Card::Pokemon(_) => CardType::Pokemon,
                    Card::Trainer(_) => CardType::Trainer,
                },
                is_basic: card.is_basic(),
                status: get_implementation_status(card_id),
                image_url: None,
            }
        })
        .collect();
    cards.sort_by(|a, b| a.id.cmp(&b.id));
    Json(cards)
}
