use rand::rngs::StdRng;
use std::fmt::Debug;

use crate::{actions::Action, Deck, State};

use super::Player;

/// Placeholder `Player` for interactive seats. `Game::step`/`submit_action`/`submit_draw`
/// drive interactive seats directly, bypassing `decision_fn` entirely; this type exists only
/// so `Game::new` has a `Box<dyn Player>` to call `get_deck()` on for that seat.
///
/// If `decision_fn` is ever invoked, the interactive-interception logic in
/// `Game::resolve_until_decision` has a bug (e.g. the seat was never registered via
/// `Game::set_interactive`, or the interactive-seat check ran after the `decision_fn` call
/// rather than before) — panic loudly rather than silently letting the seat be driven by
/// nothing. Use this (not `RandomPlayer`) for interactive seats in tests so this tripwire is
/// armed.
pub struct InteractivePlayer {
    pub deck: Deck,
}

impl Player for InteractivePlayer {
    fn get_deck(&self) -> Deck {
        self.deck.clone()
    }

    fn decision_fn(&mut self, _: &mut StdRng, _: &State, _: &[Action]) -> Action {
        panic!(
            "InteractivePlayer::decision_fn invoked directly — this seat should have been \
             intercepted by Game::step()/submit_action()/submit_draw() before reaching \
             Player::decision_fn. Likely cause: Game::set_interactive was never called for \
             this seat, or resolve_until_decision's interactive check has a bug."
        );
    }
}

impl Debug for InteractivePlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InteractivePlayer")
    }
}
