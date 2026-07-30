//! Thin wasm-bindgen wrapper around deckgym-core's interactive control plane
//! (`Game::step`/`submit_action`/`submit_draw`), for running games entirely client-side in the
//! browser. Mirrors the spirit of `src/python_bindings.rs` but is intentionally minimal for now
//! — just enough real surface to prove the wasm build pipeline and drive a hotseat game, not
//! full parity with the Python bindings.

use deckgym::{
    actions::Action,
    deck::Deck,
    game::InteractiveConfig,
    models::Card,
    players::{InteractivePlayer, Player},
    Game,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    // Turns wasm panics into real JS console errors (with a stack trace) instead of an opaque
    // "unreachable executed" trap.
    console_error_panic_hook::set_once();
}

/// A live game, driven entirely through the interactive control plane. Both seats are created
/// human-controlled and interactive by default — this is hotseat mode, this app's default (see
/// `web/SPEC.md`). AI-opponent mode is a natural follow-up: swap seat 1's `Player` for one of
/// the engine's existing bots and leave it `Scripted`.
#[wasm_bindgen]
pub struct WasmGame {
    game: Game<'static>,
}

#[wasm_bindgen]
impl WasmGame {
    /// `deck_a_text`/`deck_b_text` are decks in the existing DeckGym text format
    /// (`Deck::from_string`) — the same format used by the CLI and deckgym.com's builder, so
    /// deck text can be stored/edited as-is with no new format to invent.
    #[wasm_bindgen(constructor)]
    pub fn new(deck_a_text: &str, deck_b_text: &str, seed: u64) -> Result<WasmGame, JsError> {
        let deck_a = Deck::from_string(deck_a_text).map_err(|e| JsError::new(&e))?;
        let deck_b = Deck::from_string(deck_b_text).map_err(|e| JsError::new(&e))?;

        let player_a: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_a });
        let player_b: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_b });
        let mut game = Game::new(vec![player_a, player_b], seed);
        game.set_interactive(
            0,
            InteractiveConfig {
                override_draws: true,
            },
        );
        game.set_interactive(
            1,
            InteractiveConfig {
                override_draws: true,
            },
        );

        Ok(WasmGame { game })
    }

    /// Advances the game until the next point requiring external input (or game over).
    /// Returns a `PendingDecision` (`{kind: "awaiting_action" | "awaiting_draw" | "game_over",
    /// ...}`).
    pub fn step(&mut self) -> Result<JsValue, JsError> {
        to_js(&self.game.step())
    }

    /// Resolves an `awaiting_action` decision with one of the `actions` it returned.
    pub fn submit_action(&mut self, action: JsValue) -> Result<JsValue, JsError> {
        let action: Action = serde_wasm_bindgen::from_value(action)
            .map_err(|e| JsError::new(&format!("invalid action: {e}")))?;
        let pending = self
            .game
            .submit_action(action)
            .map_err(|e| JsError::new(&e.to_string()))?;
        to_js(&pending)
    }

    /// Resolves an `awaiting_draw` decision. Pass `undefined`/`null` to draw normally (top of
    /// deck); pass a specific card (as returned in the game's deck listing) to force that draw.
    pub fn submit_draw(&mut self, card: JsValue) -> Result<JsValue, JsError> {
        let card: Option<Card> = serde_wasm_bindgen::from_value(card)
            .map_err(|e| JsError::new(&format!("invalid card: {e}")))?;
        let pending = self
            .game
            .submit_draw(card)
            .map_err(|e| JsError::new(&e.to_string()))?;
        to_js(&pending)
    }

    /// Returns a full clone of the current game state.
    pub fn get_state(&self) -> Result<JsValue, JsError> {
        to_js(&self.game.get_state_clone())
    }
}

fn to_js<T: serde::Serialize>(value: &T) -> Result<JsValue, JsError> {
    serde_wasm_bindgen::to_value(value).map_err(|e| JsError::new(&e.to_string()))
}
