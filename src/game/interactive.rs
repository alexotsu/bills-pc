use serde::Serialize;

use crate::{
    actions::{apply_action, Action, DrawSource, SimpleAction},
    models::Card,
    simulation_event_handler::SimulationEventHandler,
    state::GameOutcome,
};

use super::Game;

/// Per-seat opt-in configuration for the interactive control plane. Attached to a `Game`
/// post-construction via `Game::set_interactive` — `Game::new`'s signature is untouched, so
/// every existing caller keeps today's fully-scripted behavior by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct InteractiveConfig {
    /// If true, this seat's `TurnStart`/`InitialHand` draws pause for an explicit
    /// `Game::submit_draw` call instead of silently resolving top-of-deck. If false, this
    /// seat still pauses with `PendingDecision::AwaitingAction` for every real (len > 1)
    /// decision, but its draws resolve normally.
    pub override_draws: bool,
    /// If false, a decision point with exactly one legal *non-draw* action (e.g. "EndTurn" is
    /// the only thing left to do, or only one Basic in hand so it's the only legal Place) still
    /// pauses with `PendingDecision::AwaitingAction` for this seat instead of applying it
    /// silently — otherwise a human player can be surprised by their turn ending, or their
    /// opening hand being played, with no visible confirmation. If true, these resolve
    /// automatically exactly like a non-interactive seat would.
    ///
    /// One exception regardless of this setting: an EndTurn that's the sole legal action
    /// *because the player just attacked* always auto-advances — attacking already is the
    /// player's decision to end their turn, by the game's own rules, so re-confirming it would
    /// be redundant (see `resolve_until_decision`).
    pub auto_advance_forced_actions: bool,
}

impl Default for InteractiveConfig {
    fn default() -> Self {
        // `auto_advance_forced_actions: true` (not derived-Default's `false`) so every existing
        // caller of `InteractiveConfig::default()` keeps behaving exactly as it did before this
        // field existed — only callers that explicitly opt out (the web frontend's own toggle)
        // get the new pause-and-confirm behavior.
        Self {
            override_draws: false,
            auto_advance_forced_actions: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SeatMode {
    /// Driven by the seat's `Player` trait object, exactly as today.
    Scripted,
    /// Driven by `Game::step`/`submit_action`/`submit_draw`; this seat's
    /// `Player::decision_fn` must never be invoked.
    Interactive(InteractiveConfig),
}

/// The next point in the game requiring external input, returned by `Game::step` and every
/// `submit_*` call.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PendingDecision {
    /// `actor` must choose one of `actions` (len >= 2). Resolve with `Game::submit_action`.
    AwaitingAction { actor: usize, actions: Vec<Action> },
    /// `actor`'s queued draw is paused for override. `amount` is always 1 today (both
    /// `InitialHand` and `TurnStart` queue one card at a time). Resolve with
    /// `Game::submit_draw`.
    AwaitingDraw {
        actor: usize,
        source: DrawSource,
        amount: u8,
    },
    /// The game has ended; mirrors `State::winner`. A struct variant (not a newtype/tuple
    /// variant) specifically because `#[serde(tag = "kind")]` (internal tagging) can't
    /// serialize a newtype variant whose payload isn't itself a JSON object — `Option<T>`
    /// isn't, so `GameOver(Option<GameOutcome>)` would fail to serialize at all the moment a
    /// game actually ended. A named field sidesteps that entirely.
    GameOver { outcome: Option<GameOutcome> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmitError {
    GameOver,
    /// The submitted action isn't a member of the current legal action set. State is left
    /// unchanged.
    IllegalAction,
    /// The current pending decision isn't a draw-override point (e.g. it's an
    /// `AwaitingAction`, or the sole pending action isn't a `DrawCard`).
    NotAwaitingDraw,
    /// The submitted card is not present in the player's remaining deck. State is left
    /// unchanged.
    CardNotInRemainingDeck,
}

impl std::fmt::Display for SubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubmitError::GameOver => write!(f, "the game has already ended"),
            SubmitError::IllegalAction => {
                write!(f, "action is not a member of the current legal action set")
            }
            SubmitError::NotAwaitingDraw => {
                write!(
                    f,
                    "the current pending decision is not a draw-override point"
                )
            }
            SubmitError::CardNotInRemainingDeck => {
                write!(f, "card is not present in the player's remaining deck")
            }
        }
    }
}

impl std::error::Error for SubmitError {}

impl<'a> Game<'a> {
    pub fn set_interactive(&mut self, seat: usize, config: InteractiveConfig) {
        self.interactive_seats[seat] = SeatMode::Interactive(config);
    }

    /// Reverts a seat to `Player`-driven control. Games can freely mix interactive and
    /// scripted seats, and a seat can move between the two mid-game (e.g. interactive only
    /// for the opening, scripted for the rest).
    pub fn set_scripted(&mut self, seat: usize) {
        self.interactive_seats[seat] = SeatMode::Scripted;
    }

    pub fn is_interactive(&self, seat: usize) -> bool {
        matches!(self.interactive_seats[seat], SeatMode::Interactive(_))
    }

    /// Runs the internal resolve loop and returns the next point requiring external input
    /// (or `GameOver`). Safe to call repeatedly / after `set_state`.
    pub fn step(&mut self) -> PendingDecision {
        self.resolve_until_decision()
    }

    pub fn submit_action(&mut self, action: Action) -> Result<PendingDecision, SubmitError> {
        if self.state.is_game_over() {
            return Err(SubmitError::GameOver);
        }
        let (actor, actions) = self.state.generate_possible_actions();
        if !actions.contains(&action) {
            return Err(SubmitError::IllegalAction);
        }
        self.fire_event_and_apply(actor, &actions, &action);
        Ok(self.resolve_until_decision())
    }

    /// `card: None` draws normally (top of remaining deck — identical to what would have
    /// happened without an override). `card: Some(c)` moves `c` to the front of the deck
    /// first, then draws, so the existing `apply_action -> maybe_draw_card` pipeline
    /// (hand-cap check included) runs completely unmodified.
    ///
    /// Note: if the seat's hand is already at the 10-card cap, the reordered card still ends
    /// up at the front of the deck (matching `maybe_draw_card`'s existing silent no-op at
    /// cap) and is simply drawn next time there's room — no special-cased error for this.
    pub fn submit_draw(&mut self, card: Option<Card>) -> Result<PendingDecision, SubmitError> {
        if self.state.is_game_over() {
            return Err(SubmitError::GameOver);
        }
        let (actor, actions) = self.state.generate_possible_actions();
        let [action] = actions.as_slice() else {
            return Err(SubmitError::NotAwaitingDraw);
        };
        if !matches!(action.action, SimpleAction::DrawCard { .. }) {
            return Err(SubmitError::NotAwaitingDraw);
        }
        if let Some(card) = card {
            self.state
                .move_card_to_front_of_deck(actor, &card)
                .map_err(|_| SubmitError::CardNotInRemainingDeck)?;
        }
        let action = action.clone();
        self.fire_event_and_apply(actor, &actions, &action);
        Ok(self.resolve_until_decision())
    }

    /// Shared by `step`/`submit_action`/`submit_draw`. Auto-resolves every non-interactive
    /// decision point (single-action auto-select, else `decision_fn` — identical semantics
    /// to `play_tick`, including firing the same event hook, so export is bit-for-bit
    /// identical whether the game was bulk-played or interactively driven). Returns control
    /// the moment an interactive seat has a real decision, or an interactive+override-enabled
    /// seat has a pending draw.
    fn resolve_until_decision(&mut self) -> PendingDecision {
        loop {
            if self.state.is_game_over() {
                return PendingDecision::GameOver {
                    outcome: self.state.winner,
                };
            }
            let (actor, actions) = self.state.generate_possible_actions();

            if actions.len() == 1 {
                if let SimpleAction::DrawCard { amount, source } = actions[0].action {
                    if self.wants_draw_override(actor, source) {
                        return PendingDecision::AwaitingDraw {
                            actor,
                            source,
                            amount,
                        };
                    }
                } else if actions[0].action == SimpleAction::EndTurn
                    && self.state.attack_name_used_this_turn[actor].is_some()
                {
                    // Attacking ends your turn by the game's own rules — the player already
                    // made that call when they chose to attack, so pausing again just to
                    // confirm the EndTurn that inevitably follows would be redundant, not
                    // informative. This is the one case that auto-advances regardless of
                    // `auto_advance_forced_actions`; every *other* forced single action still
                    // honors it.
                } else if self.wants_forced_action_confirmation(actor) {
                    return PendingDecision::AwaitingAction { actor, actions };
                }
            } else if self.is_interactive(actor) {
                return PendingDecision::AwaitingAction { actor, actions };
            }

            let action = if actions.len() == 1 {
                actions[0].clone()
            } else {
                // actor is guaranteed Scripted here (Interactive returned above)
                self.players[actor].decision_fn(&mut self.rng, &self.state, &actions)
            };
            self.fire_event_and_apply(actor, &actions, &action);
        }
    }

    fn fire_event_and_apply(&mut self, actor: usize, actions: &[Action], action: &Action) {
        if let Some(handler) = &mut self.event_handler {
            handler.on_action(self.id, &self.state, actor, actions, action);
        }
        apply_action(&mut self.rng, &mut self.state, action);
    }

    fn wants_draw_override(&self, actor: usize, source: DrawSource) -> bool {
        matches!(source, DrawSource::TurnStart | DrawSource::InitialHand)
            && matches!(
                self.interactive_seats[actor],
                SeatMode::Interactive(cfg) if cfg.override_draws
            )
    }

    fn wants_forced_action_confirmation(&self, actor: usize) -> bool {
        matches!(
            self.interactive_seats[actor],
            SeatMode::Interactive(cfg) if !cfg.auto_advance_forced_actions
        )
    }
}
