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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct InteractiveConfig {
    /// If true, this seat's `TurnStart`/`InitialHand` draws pause for an explicit
    /// `Game::submit_draw` call instead of silently resolving top-of-deck. If false, this
    /// seat still pauses with `PendingDecision::AwaitingAction` for every real (len > 1)
    /// decision, but its draws resolve normally.
    pub override_draws: bool,
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
    /// The game has ended; mirrors `State::winner`.
    GameOver(Option<GameOutcome>),
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
                return PendingDecision::GameOver(self.state.winner);
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
}
