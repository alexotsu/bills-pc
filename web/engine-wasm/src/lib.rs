//! Thin wasm-bindgen wrapper around deckgym-core's interactive control plane
//! (`Game::step`/`submit_action`/`submit_draw`), for running games entirely client-side in the
//! browser. Mirrors the spirit of `src/python_bindings.rs` but is intentionally minimal for now
//! — just enough real surface to prove the wasm build pipeline and drive a hotseat game, not
//! full parity with the Python bindings.

use deckgym::{
    actions::Action,
    deck::Deck,
    game::{InteractiveConfig, PendingDecision, SubmitError},
    models::Card,
    players::{InteractivePlayer, Player},
    state::GameOutcome,
    Game, State,
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
///
/// The state-management logic (`new_inner`/`submit_action_inner`/`undo_inner`/...) is kept in a
/// plain, non-wasm_bindgen `impl` block operating only on native `deckgym` types, so it can be
/// covered by ordinary `#[test]`s below — `wasm_bindgen`'s `JsValue` marshalling isn't available
/// outside a real wasm+JS environment (that needs `wasm-pack test`, not plain `cargo test`), so
/// keeping it out of the tested logic entirely is what makes native testing possible at all. The
/// `#[wasm_bindgen] impl` further down is just JsValue <-> native conversion around these.
#[wasm_bindgen]
pub struct WasmGame {
    game: Game<'static>,
    /// One snapshot per successfully-applied `submit_action`/`submit_draw`, taken *before* that
    /// call — same pattern as `AppMode::Interactive`'s `state_history` in `src/tui/app.rs`.
    /// `undo()` pops the last one and restores it via `Game::set_state`.
    state_history: Vec<State>,
}

impl WasmGame {
    /// `deck_a_text`/`deck_b_text` are decks in the existing DeckGym text format
    /// (`Deck::from_string`) — the same format used by the CLI and deckgym.com's builder, so
    /// deck text can be stored/edited as-is with no new format to invent.
    ///
    /// `override_draws` and `auto_advance_forced_actions` both apply to both seats equally
    /// (this is hotseat — one person is behind both). `override_draws` is off by default in the
    /// frontend's own game-setup UI, since most players want plain random draws and reserve
    /// manual picking for the rarer case they explicitly want to simulate a specific opening or
    /// top-deck. `auto_advance_forced_actions` is also off by default in the frontend, so a
    /// forced single legal action (e.g. "EndTurn" is the only thing left to do, or only one
    /// Basic in hand) always pauses for confirmation rather than resolving silently.
    /// `starting_player` forces who takes the first turn: `0` or `1`; anything else (frontend
    /// convention: `-1`) leaves it to the engine's own seed-driven coin flip
    /// (`State::initialize`, `src/state/mod.rs`).
    fn new_inner(
        deck_a_text: &str,
        deck_b_text: &str,
        seed: u64,
        override_draws: bool,
        starting_player: i32,
        auto_advance_forced_actions: bool,
    ) -> Result<WasmGame, String> {
        let deck_a = Deck::from_string(deck_a_text)?;
        let deck_b = Deck::from_string(deck_b_text)?;

        let player_a: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_a });
        let player_b: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_b });
        let mut game = Game::new(vec![player_a, player_b], seed);
        let config = InteractiveConfig {
            override_draws,
            auto_advance_forced_actions,
        };
        game.set_interactive(0, config);
        game.set_interactive(1, config);

        // Applied *after* construction, directly on the cloned state: the opening-hand draw
        // order (see State::initialize) is already fixed at [0,1,0,1,...] independently of
        // current_player, so overriding just this one field doesn't disturb anything else the
        // coin flip would otherwise have set up (deck shuffles, opening hands, energy zone).
        if starting_player == 0 || starting_player == 1 {
            let mut state = game.get_state_clone();
            state.current_player = starting_player as usize;
            game.set_state(state);
        }

        Ok(WasmGame {
            game,
            state_history: Vec::new(),
        })
    }

    fn submit_action_inner(&mut self, action: Action) -> Result<PendingDecision, SubmitError> {
        let snapshot = self.game.get_state_clone();
        let pending = self.game.submit_action(action)?;
        // Only recorded once the action is confirmed legal — an illegal action leaves state
        // (and so state_history) untouched, matching SubmitError::IllegalAction's contract.
        self.state_history.push(snapshot);
        Ok(pending)
    }

    fn submit_draw_inner(&mut self, card: Option<Card>) -> Result<PendingDecision, SubmitError> {
        let snapshot = self.game.get_state_clone();
        let pending = self.game.submit_draw(card)?;
        self.state_history.push(snapshot);
        Ok(pending)
    }

    /// Reverts the last successfully-applied `submit_action`/`submit_draw` and returns the
    /// `PendingDecision` at that earlier point. `None` if there's nothing to undo.
    fn undo_inner(&mut self) -> Option<PendingDecision> {
        let previous = self.state_history.pop()?;
        self.game.set_state(previous);
        // `Game::step` is documented safe to call repeatedly / after `set_state` — it just
        // re-resolves from whatever the current state is, so this correctly recomputes the
        // PendingDecision (and re-derives draw-override eligibility) for the restored state
        // without needing to duplicate that resolution logic here.
        Some(self.game.step())
    }

    fn can_undo_inner(&self) -> bool {
        !self.state_history.is_empty()
    }

    /// Force-ends the game with a specific outcome, for a player who wants to concede or
    /// otherwise call the game manually (e.g. via the frontend's "Declare Winner" button)
    /// rather than play it out. The current state is pushed onto `state_history` first, so an
    /// accidental declaration is undoable exactly like any other step.
    fn declare_winner_inner(&mut self, outcome: GameOutcome) -> PendingDecision {
        let snapshot = self.game.get_state_clone();
        let mut state = snapshot.clone();
        state.winner = Some(outcome);
        self.game.set_state(state);
        self.state_history.push(snapshot);
        self.game.step()
    }
}

#[wasm_bindgen]
impl WasmGame {
    #[wasm_bindgen(constructor)]
    pub fn new(
        deck_a_text: &str,
        deck_b_text: &str,
        seed: u64,
        override_draws: bool,
        starting_player: i32,
        auto_advance_forced_actions: bool,
    ) -> Result<WasmGame, JsError> {
        Self::new_inner(
            deck_a_text,
            deck_b_text,
            seed,
            override_draws,
            starting_player,
            auto_advance_forced_actions,
        )
        .map_err(|e| JsError::new(&e))
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
            .submit_action_inner(action)
            .map_err(|e| JsError::new(&e.to_string()))?;
        to_js(&pending)
    }

    /// Resolves an `awaiting_draw` decision. Pass `undefined`/`null` to draw normally (top of
    /// deck); pass a specific card (as returned in the game's deck listing) to force that draw.
    pub fn submit_draw(&mut self, card: JsValue) -> Result<JsValue, JsError> {
        let card: Option<Card> = serde_wasm_bindgen::from_value(card)
            .map_err(|e| JsError::new(&format!("invalid card: {e}")))?;
        let pending = self
            .submit_draw_inner(card)
            .map_err(|e| JsError::new(&e.to_string()))?;
        to_js(&pending)
    }

    pub fn undo(&mut self) -> Result<JsValue, JsError> {
        let pending = self
            .undo_inner()
            .ok_or_else(|| JsError::new("nothing to undo"))?;
        to_js(&pending)
    }

    pub fn can_undo(&self) -> bool {
        self.can_undo_inner()
    }

    /// Force-ends the game with a specific outcome (`{"Win": 0}`, `{"Win": 1}`, or `"Tie"`) —
    /// for a "Declare Winner" UI, not something reachable through normal play. Undoable like any
    /// other step.
    pub fn declare_winner(&mut self, outcome: JsValue) -> Result<JsValue, JsError> {
        let outcome: GameOutcome = serde_wasm_bindgen::from_value(outcome)
            .map_err(|e| JsError::new(&format!("invalid outcome: {e}")))?;
        to_js(&self.declare_winner_inner(outcome))
    }

    /// Returns a full clone of the current game state.
    pub fn get_state(&self) -> Result<JsValue, JsError> {
        to_js(&self.game.get_state_clone())
    }
}

fn to_js<T: serde::Serialize>(value: &T) -> Result<JsValue, JsError> {
    serde_wasm_bindgen::to_value(value).map_err(|e| JsError::new(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Small, real decks in the DeckGym text format (same ones used in
    // web/frontend/src/app/scaffold-check/page.tsx).
    const DECK_A: &str = "Pokémon: 10
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
";

    const DECK_B: &str = "Pokémon: 8
2 Ekans A1 164
2 Arbok A1 165
2 Koffing A1 176
2 Weezing A1 177

Trainer: 12
2 Professor's Research P-A 007
2 Koga A1 222
2 Poké Ball P-A 005
2 Sabrina A1 225
2 Potion P-A 001
1 X Speed P-A 002
1 Giovanni A1 223
";

    fn new_game() -> WasmGame {
        WasmGame::new_inner(DECK_A, DECK_B, 42, true, -1, true)
            .expect("decks should parse and start a game")
    }

    /// Drives the game forward one decision by always picking the first legal action —
    /// deliberately dumb, just needs *a* real state transition to snapshot/undo around.
    fn apply_first_legal_action(game: &mut WasmGame) -> PendingDecision {
        let pending = game.game.step();
        match pending {
            PendingDecision::AwaitingAction { actions, .. } => {
                let action = actions[0].clone();
                game.submit_action_inner(action)
                    .expect("first legal action should be legal")
            }
            PendingDecision::AwaitingDraw { .. } => game
                .submit_draw_inner(None)
                .expect("normal draw should always be legal"),
            PendingDecision::GameOver { .. } => pending,
        }
    }

    #[test]
    fn cannot_undo_a_fresh_game() {
        let game = new_game();
        assert!(!game.can_undo_inner());
    }

    #[test]
    fn undo_reverts_state_and_pending_decision_to_before_the_last_submit() {
        // `new_game()` is deterministic (fixed seed, fixed decks), so a second, never-advanced
        // instance's first `step()` is exactly the decision `game` faced before its own first
        // submit — the baseline `pending_after_undo` must match.
        let mut fresh = new_game();
        let pending_before = fresh.game.step();

        let mut game = new_game();
        let state_before = game.game.get_state_clone();
        apply_first_legal_action(&mut game);
        assert!(game.can_undo_inner());
        assert_ne!(game.game.get_state_clone(), state_before);

        let pending_after_undo = game.undo_inner().expect("should have one step to undo");

        assert_eq!(game.game.get_state_clone(), state_before);
        assert!(!game.can_undo_inner());
        assert_eq!(pending_after_undo, pending_before);
    }

    #[test]
    fn undo_on_a_fresh_game_returns_none() {
        let mut game = new_game();
        assert!(game.undo_inner().is_none());
    }

    #[test]
    fn multiple_undos_pop_history_in_order() {
        let mut game = new_game();
        let state_0 = game.game.get_state_clone();
        apply_first_legal_action(&mut game);
        let state_1 = game.game.get_state_clone();
        apply_first_legal_action(&mut game);
        assert_ne!(state_1, game.game.get_state_clone());

        game.undo_inner().unwrap();
        assert_eq!(game.game.get_state_clone(), state_1);
        game.undo_inner().unwrap();
        assert_eq!(game.game.get_state_clone(), state_0);
        assert!(!game.can_undo_inner());
    }

    #[test]
    fn illegal_action_does_not_grow_history() {
        let mut game = new_game();
        // `Noop` is never a member of any real legal-action set (see SimpleAction's doc comment
        // in src/actions/types.rs), so this is rejected regardless of which decision — draw or
        // action — happens to be first for this seed/decks; the rejection itself, not reaching
        // any particular decision, is what this test is about.
        let bogus = Action {
            actor: 0,
            action: deckgym::actions::SimpleAction::Noop,
            is_stack: false,
        };

        assert!(game.submit_action_inner(bogus).is_err());
        assert!(!game.can_undo_inner());
    }

    #[test]
    fn starting_player_override_forces_who_goes_first() {
        let mut forced_zero =
            WasmGame::new_inner(DECK_A, DECK_B, 42, true, 0, true).expect("should construct");
        assert_eq!(forced_zero.game.get_state_clone().current_player, 0);

        let mut forced_one =
            WasmGame::new_inner(DECK_A, DECK_B, 42, true, 1, true).expect("should construct");
        assert_eq!(forced_one.game.get_state_clone().current_player, 1);

        // Forcing shouldn't disturb the fixed opening-hand draw order (still actor 0 first).
        let PendingDecision::AwaitingDraw { actor, .. } = forced_zero.game.step() else {
            panic!("expected the opening hand draw to still be pending");
        };
        assert_eq!(actor, 0);
        let PendingDecision::AwaitingDraw { actor, .. } = forced_one.game.step() else {
            panic!("expected the opening hand draw to still be pending");
        };
        assert_eq!(actor, 0);
    }

    #[test]
    fn override_draws_false_skips_straight_past_the_opening_hand() {
        // With override_draws off, none of the 10 opening-hand draws (5 per seat) should pause
        // for a choice — step() should resolve all the way through to the first real decision.
        let mut game =
            WasmGame::new_inner(DECK_A, DECK_B, 42, false, -1, true).expect("should construct");
        let pending = game.game.step();
        assert!(
            !matches!(pending, PendingDecision::AwaitingDraw { .. }),
            "expected no draw pause with override_draws=false, got {pending:?}"
        );
    }

    #[test]
    fn auto_advance_forced_actions_false_pauses_on_forced_single_action() {
        // Confirms the wasm wrapper actually threads this parameter through to
        // InteractiveConfig — the underlying engine behavior itself has its own dedicated
        // coverage in tests/engine/interactive_test.rs.
        let mut game =
            WasmGame::new_inner(DECK_A, DECK_B, 42, true, -1, false).expect("should construct");

        let mut state = game.game.get_state_clone();
        state.move_generation_stack.clear();
        state.current_player = 0;
        let bulbasaur =
            deckgym::database::get_card_by_enum(deckgym::card_ids::CardId::A1001Bulbasaur);
        state.hands[0].clear();
        state.hands[0].push(bulbasaur.clone());
        state.hands[1].clear();
        state.hands[1].push(bulbasaur);
        game.game.set_state(state);

        match game.game.step() {
            PendingDecision::AwaitingAction { actor, actions } => {
                assert_eq!(actor, 0);
                assert_eq!(actions.len(), 1);
            }
            other => panic!("expected a paused single-action AwaitingAction, got {other:?}"),
        }
    }

    #[test]
    fn declare_winner_force_ends_the_game_and_is_undoable() {
        let mut game = new_game();
        let state_before = game.game.get_state_clone();

        let pending = game.declare_winner_inner(GameOutcome::Win(1));
        assert_eq!(
            pending,
            PendingDecision::GameOver {
                outcome: Some(GameOutcome::Win(1))
            }
        );
        assert_eq!(
            game.game.get_state_clone().winner,
            Some(GameOutcome::Win(1))
        );
        assert!(game.can_undo_inner());

        let undone = game.undo_inner().expect("should have one step to undo");
        assert_eq!(game.game.get_state_clone(), state_before);
        assert_ne!(undone, PendingDecision::GameOver { outcome: None });
    }

    #[test]
    fn declare_winner_supports_tie() {
        let mut game = new_game();
        let pending = game.declare_winner_inner(GameOutcome::Tie);
        assert_eq!(
            pending,
            PendingDecision::GameOver {
                outcome: Some(GameOutcome::Tie)
            }
        );
    }
}
