use std::sync::{Arc, Mutex};

use deckgym::{
    actions::{Action, DrawSource, SimpleAction},
    card_ids::CardId,
    database::get_card_by_enum,
    game::{InteractiveConfig, PendingDecision, SubmitError},
    models::{Card, EnergyType},
    players::{AttachAttackPlayer, InteractivePlayer, Player, RandomPlayer},
    simulation_event_handler::{CompositeSimulationEventHandler, SimulationEventHandler},
    state::GameOutcome,
    test_support::{init_random_players, load_test_decks},
    Game, State,
};
use uuid::Uuid;

fn find_action<F>(game: &Game, predicate: F) -> Action
where
    F: Fn(&Action) -> bool,
{
    let (_actor, actions) = game.get_state_clone().generate_possible_actions();
    actions
        .into_iter()
        .find(predicate)
        .expect("expected action to be available")
}

/// Regression test for a latent bug fixed alongside the interactive control plane:
/// `generate_possible_actions` used to check `turn_count == 0` (setup phase) before
/// checking `move_generation_stack`, so a forced follow-up decision pushed during setup
/// (e.g. Miraidon ex's Legendary Drive switch-to-active offer, triggered by benching it)
/// was silently dropped. It only resurfaced later, once turn_count reached 1, as a phantom
/// decision ahead of real turn-1 play.
#[test]
fn test_legendary_drive_offered_during_setup_phase() {
    let players = init_random_players();
    let mut game = Game::new(players, 0);

    let mut state = game.get_state_clone();
    assert_eq!(state.turn_count, 0, "game should still be in setup phase");
    // The 10 opening-hand draws are queued on the stack rather than resolved synchronously
    // (see DrawSource::InitialHand); clear them since this test injects a hand directly and
    // wants to jump straight to setup-phase Place decisions.
    state.move_generation_stack.clear();
    state.hands[state.current_player].clear();
    state.hands[state.current_player].push(get_card_by_enum(CardId::A1001Bulbasaur));
    state.hands[state.current_player].push(get_card_by_enum(CardId::B3a019MiraidonEx));
    game.set_state(state);

    // Place the active Pokemon first (required before any bench placement is offered).
    let place_active = find_action(&game, |a| matches!(a.action, SimpleAction::Place(_, 0)));
    game.apply_action(&place_active);

    // Bench Miraidon ex (Legendary Drive triggers "on bench from hand").
    let place_miraidon = find_action(
        &game,
        |a| matches!(a.action, SimpleAction::Place(ref c, pos) if pos != 0 && c.get_name() == "Miraidon ex"),
    );
    game.apply_action(&place_miraidon);

    // Still turn_count == 0: the switch-to-active choice must be surfaced immediately,
    // not dropped until turn 1.
    let state = game.get_state_clone();
    assert_eq!(state.turn_count, 0);
    let (_actor, actions) = state.generate_possible_actions();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a.action, SimpleAction::UseAbility { .. })),
        "Legendary Drive should be offered immediately after benching during setup, got: {:?}",
        actions.iter().map(|a| &a.action).collect::<Vec<_>>()
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a.action, SimpleAction::Noop)),
        "Noop should be offered alongside Legendary Drive"
    );
}

/// `State::initialize`'s 10 opening-hand draws were restructured from a synchronous loop
/// into queued, tick-resolved `SimpleAction::DrawCard { source: InitialHand }` actions (so
/// an interactive seat can override them). `Deck::draw`/`maybe_draw_card` consume no RNG,
/// and `DrawCard` is a deterministic single-outcome action so applying it via `apply_action`
/// also consumes no RNG — so this restructuring must not shift the RNG stream relative to
/// `shuffle`/the starting-player roll/the energy rolls. These golden values were captured
/// against the pre-restructuring implementation for seed 12345 and must still match exactly.
#[test]
fn test_initial_hand_rng_stream_unchanged() {
    let players = init_random_players();
    let mut game = Game::new(players, 12345);

    // current_player/energy_zone are set synchronously in `initialize`, before any of the
    // 10 queued opening-hand draws resolve, so they're already correct here.
    let state = game.get_state_clone();
    assert_eq!(state.current_player, 1);
    assert_eq!(state.energy_zone[0].next, Some(EnergyType::Grass));
    assert_eq!(state.energy_zone[1].next, Some(EnergyType::Darkness));

    // Drain exactly the 10 queued draws (each a single-action tick, auto-selected, no RNG
    // and no player decision involved) without entering the setup-phase Place decisions.
    while !game.get_state_clone().move_generation_stack.is_empty() {
        game.play_tick();
    }
    let state = game.get_state_clone();
    assert_eq!(
        card_names(&state.hands[0]),
        vec![
            "Exeggcute",
            "Sabrina",
            "Professor's Research",
            "Ivysaur",
            "Poké Ball"
        ]
    );
    assert_eq!(
        card_names(&state.hands[1]),
        vec!["Ekans", "Potion", "Ekans", "Arbok", "Giovanni"]
    );
    assert_eq!(state.decks[0].cards.len(), 15);
    assert_eq!(state.decks[1].cards.len(), 15);
}

fn card_names(cards: &[deckgym::models::Card]) -> Vec<String> {
    cards.iter().map(|c| c.get_name()).collect()
}

/// Drives an interactive seat 0 to completion, submitting the first available action for
/// every `AwaitingAction` and the default (top-of-deck) card for every `AwaitingDraw`. Panics
/// if the game doesn't terminate within a generous ply budget.
fn drive_seat_zero_to_completion(game: &mut Game) -> Option<GameOutcome> {
    let mut pending = game.step();
    for _ in 0..10_000 {
        pending = match pending {
            PendingDecision::AwaitingAction { actions, .. } => game
                .submit_action(actions[0].clone())
                .expect("submit_action should succeed"),
            PendingDecision::AwaitingDraw { .. } => {
                game.submit_draw(None).expect("submit_draw should succeed")
            }
            PendingDecision::GameOver { outcome } => return outcome,
        };
    }
    panic!("game did not terminate within the ply budget");
}

#[test]
fn test_interactive_seat_awaiting_action_and_submit_resumes_game() {
    let (deck_a, deck_b) = load_test_decks();
    let player_a: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_a });
    let player_b: Box<dyn Player> = Box::new(RandomPlayer { deck: deck_b });
    let mut game = Game::new(vec![player_a, player_b], 7);
    game.set_interactive(0, InteractiveConfig::default());

    let outcome = drive_seat_zero_to_completion(&mut game);
    assert!(outcome.is_some(), "game should not time out");
}

#[test]
fn test_turn_start_draw_override_picks_non_top_of_deck_card() {
    let (deck_a, deck_b) = load_test_decks();
    let player_a: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_a });
    let player_b: Box<dyn Player> = Box::new(RandomPlayer { deck: deck_b });
    let mut game = Game::new(vec![player_a, player_b], 7);
    game.set_interactive(
        0,
        InteractiveConfig {
            override_draws: true,
            ..Default::default()
        },
    );

    // Drive forward, auto-resolving everything, until player 0's first *TurnStart* draw
    // (as opposed to one of its 5 InitialHand draws, which also pause since override_draws
    // is on) — stop right there without submitting it, so we can inspect + override it.
    let mut pending = game.step();
    let mut iterations = 0;
    loop {
        match pending {
            PendingDecision::AwaitingDraw {
                actor: 0,
                source: DrawSource::TurnStart,
                ..
            } => break,
            PendingDecision::AwaitingDraw { .. } => {
                pending = game.submit_draw(None).expect("submit_draw should succeed");
            }
            PendingDecision::AwaitingAction { ref actions, .. } => {
                let action = actions[0].clone();
                pending = game
                    .submit_action(action)
                    .expect("submit_action should succeed");
            }
            PendingDecision::GameOver { .. } => {
                panic!("game ended before player 0's TurnStart draw")
            }
        }
        iterations += 1;
        assert!(
            iterations < 1_000,
            "did not reach a TurnStart draw for player 0 in time"
        );
    }

    let state_before = game.get_state_clone();
    let deck_size_before = state_before.decks[0].cards.len();
    let chosen_card: Card = state_before.decks[0].cards[3].clone();
    assert_ne!(
        chosen_card, state_before.decks[0].cards[0],
        "test setup should pick a card that isn't already on top"
    );

    game.submit_draw(Some(chosen_card.clone()))
        .expect("submit_draw should succeed");

    let state_after = game.get_state_clone();
    assert!(
        state_after.hands[0].contains(&chosen_card),
        "the chosen card should be drawn into hand"
    );
    assert_eq!(state_after.decks[0].cards.len(), deck_size_before - 1);
}

#[test]
fn test_initial_hand_draw_override_produces_exact_chosen_hand() {
    let (deck_a, deck_b) = load_test_decks();
    let player_a: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_a });
    let player_b: Box<dyn Player> = Box::new(RandomPlayer { deck: deck_b });
    let mut game = Game::new(vec![player_a, player_b], 11);
    game.set_interactive(
        0,
        InteractiveConfig {
            override_draws: true,
            ..Default::default()
        },
    );

    let mut chosen: Vec<Card> = Vec::new();
    let mut pending = game.step();
    loop {
        match pending {
            PendingDecision::AwaitingDraw {
                actor: 0,
                source: DrawSource::InitialHand,
                ..
            } => {
                let state = game.get_state_clone();
                // Pick a card that isn't already at the front, proving the override actually
                // changes what gets drawn (not just re-confirming the default).
                let candidate = state.decks[0].cards[2].clone();
                chosen.push(candidate.clone());
                pending = game
                    .submit_draw(Some(candidate))
                    .expect("submit_draw should succeed");
                if chosen.len() == 5 {
                    // `submit_draw` doesn't stop right after applying the draw: its internal
                    // resolve loop keeps auto-applying anything with no real choice, which can
                    // include a forced setup-phase Place if this hand happens to contain
                    // exactly one basic Pokemon. So by the time we get here, one of the 5
                    // chosen cards may already be on the board rather than still in hand.
                    break;
                }
            }
            PendingDecision::AwaitingDraw { .. } => {
                pending = game.submit_draw(None).expect("submit_draw should succeed");
            }
            PendingDecision::AwaitingAction { ref actions, .. } => {
                let action = actions[0].clone();
                pending = game
                    .submit_action(action)
                    .expect("submit_action should succeed");
            }
            PendingDecision::GameOver { .. } => panic!("game ended before opening hand was drawn"),
        }
    }

    let state = game.get_state_clone();
    // Card values aren't unique (a deck can carry 2 copies of the same card), so checking
    // deck membership by value isn't reliable; instead confirm the deck shrank by exactly the
    // 5 draws, and that every chosen card landed either in hand or already placed in play.
    assert_eq!(state.decks[0].cards.len(), 15);
    for card in &chosen {
        let in_hand = state.hands[0].contains(card);
        let in_play = state.in_play_pokemon[0]
            .iter()
            .flatten()
            .any(|played| &played.card == card);
        assert!(
            in_hand || in_play,
            "chosen card {} should be in hand or already placed in play",
            card.get_name()
        );
    }
    assert_eq!(state.hands[1].len(), 5);
}

#[test]
fn test_mixed_interactive_and_ai_seat_full_game() {
    let (deck_a, deck_b) = load_test_decks();
    let player_a: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_a });
    let player_b: Box<dyn Player> = Box::new(AttachAttackPlayer { deck: deck_b });
    let mut game = Game::new(vec![player_a, player_b], 3);
    game.set_interactive(0, InteractiveConfig::default());

    let outcome = drive_seat_zero_to_completion(&mut game);
    assert!(
        matches!(outcome, Some(GameOutcome::Win(_)) | Some(GameOutcome::Tie)),
        "expected a valid game outcome, got {outcome:?}"
    );
}

#[derive(Default)]
struct ActionRecorder {
    log: Arc<Mutex<Vec<(usize, SimpleAction)>>>,
}

impl SimulationEventHandler for ActionRecorder {
    fn on_action(
        &mut self,
        _game_id: Uuid,
        _state_before_action: &State,
        actor: usize,
        _playable_actions: &[Action],
        action: &Action,
    ) {
        self.log
            .lock()
            .unwrap()
            .push((actor, action.action.clone()));
    }

    fn merge(&mut self, _other: &dyn SimulationEventHandler) {}
}

/// Interactively-driven games must export identically to bulk-simulated ones: the new
/// `submit_action`/`submit_draw` methods fire the same `event_handler.on_action` hook that
/// `play_tick` does (this is what `DataExporter` relies on). Uses `AttachAttackPlayer` for
/// both seats in the bulk game because its `decision_fn` ignores rng entirely, so replaying
/// its exact choices in the interactive game (from the same seed) reproduces an identical rng
/// stream and thus an identical recorded ply sequence.
#[test]
fn test_export_parity_for_interactive_game_matches_bulk_play() {
    let seed = 42;
    let (deck_a, deck_b) = load_test_decks();

    let bulk_log = Arc::new(Mutex::new(Vec::new()));
    {
        let player_a: Box<dyn Player> = Box::new(AttachAttackPlayer {
            deck: deck_a.clone(),
        });
        let player_b: Box<dyn Player> = Box::new(AttachAttackPlayer {
            deck: deck_b.clone(),
        });
        let mut handler = CompositeSimulationEventHandler::new(vec![Box::new(ActionRecorder {
            log: bulk_log.clone(),
        })]);
        let mut game = Game::new_with_event_handlers(
            Uuid::new_v4(),
            vec![player_a, player_b],
            seed,
            &mut handler,
        );
        game.play();
    }

    let interactive_log = Arc::new(Mutex::new(Vec::new()));
    {
        let player_a: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_a });
        let player_b: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_b });
        let mut handler = CompositeSimulationEventHandler::new(vec![Box::new(ActionRecorder {
            log: interactive_log.clone(),
        })]);
        let mut game = Game::new_with_event_handlers(
            Uuid::new_v4(),
            vec![player_a, player_b],
            seed,
            &mut handler,
        );
        game.set_interactive(0, InteractiveConfig::default());
        game.set_interactive(1, InteractiveConfig::default());

        // Reproduce exactly what AttachAttackPlayer would have chosen at every decision, so
        // the two games consume the same rng stream in the same order.
        let mut attach_attack_bot = AttachAttackPlayer {
            deck: load_test_decks().0,
        };
        let mut pending = game.step();
        loop {
            pending = match pending {
                PendingDecision::AwaitingAction { actor: _, actions } => {
                    use rand::SeedableRng;
                    let mut dummy_rng = rand::rngs::StdRng::seed_from_u64(0);
                    let chosen = attach_attack_bot.decision_fn(
                        &mut dummy_rng,
                        &game.get_state_clone(),
                        &actions,
                    );
                    game.submit_action(chosen)
                        .expect("submit_action should succeed")
                }
                PendingDecision::AwaitingDraw { .. } => {
                    game.submit_draw(None).expect("submit_draw should succeed")
                }
                PendingDecision::GameOver { .. } => break,
            };
        }
    }

    let bulk = bulk_log.lock().unwrap();
    let interactive = interactive_log.lock().unwrap();
    assert_eq!(
        *bulk, *interactive,
        "interactive-driven game should export the exact same (actor, action) ply sequence as bulk play"
    );
    assert!(
        !bulk.is_empty(),
        "sanity check: the game should have taken at least one action"
    );
}

#[test]
fn test_submit_draw_errors_when_card_not_in_deck() {
    let (deck_a, deck_b) = load_test_decks();
    let player_a: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_a });
    let player_b: Box<dyn Player> = Box::new(RandomPlayer { deck: deck_b });
    let mut game = Game::new(vec![player_a, player_b], 7);
    game.set_interactive(
        0,
        InteractiveConfig {
            override_draws: true,
            ..Default::default()
        },
    );

    // Advance to player 0's first AwaitingDraw.
    let mut pending = game.step();
    loop {
        match pending {
            PendingDecision::AwaitingDraw { actor: 0, .. } => break,
            PendingDecision::AwaitingDraw { .. } => {
                pending = game.submit_draw(None).expect("submit_draw should succeed");
            }
            PendingDecision::AwaitingAction { ref actions, .. } => {
                let action = actions[0].clone();
                pending = game
                    .submit_action(action)
                    .expect("submit_action should succeed");
            }
            PendingDecision::GameOver { .. } => panic!("game ended before player 0's first draw"),
        }
    }

    let state_before = game.get_state_clone();
    let card_not_in_deck = get_card_by_enum(CardId::B3a019MiraidonEx);
    assert!(
        !state_before.decks[0].cards.contains(&card_not_in_deck),
        "test setup: this card must not already be in the test deck"
    );

    let result = game.submit_draw(Some(card_not_in_deck));
    assert_eq!(result, Err(SubmitError::CardNotInRemainingDeck));
    assert_eq!(
        game.get_state_clone(),
        state_before,
        "state must be unchanged after a failed submit_draw"
    );
}

#[test]
fn test_submit_action_rejects_illegal_action() {
    let (deck_a, deck_b) = load_test_decks();
    let player_a: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_a });
    let player_b: Box<dyn Player> = Box::new(RandomPlayer { deck: deck_b });
    let mut game = Game::new(vec![player_a, player_b], 7);
    game.set_interactive(0, InteractiveConfig::default());

    // Drive until we hit an AwaitingAction for player 0.
    let mut pending = game.step();
    loop {
        match pending {
            PendingDecision::AwaitingAction { .. } => break,
            PendingDecision::AwaitingDraw { .. } => {
                pending = game.submit_draw(None).expect("submit_draw should succeed");
            }
            PendingDecision::GameOver { .. } => panic!("game ended before any AwaitingAction"),
        }
    }

    let state_before = game.get_state_clone();
    let bogus_action = Action {
        actor: 0,
        action: SimpleAction::Retreat(99),
        is_stack: false,
    };

    let result = game.submit_action(bogus_action);
    assert_eq!(result, Err(SubmitError::IllegalAction));
    assert_eq!(
        game.get_state_clone(),
        state_before,
        "state must be unchanged after a failed submit_action"
    );
}

/// With a single Basic in hand, placing it as Active is the only legal action — with
/// `auto_advance_forced_actions: false`, that must still pause for confirmation rather than
/// being silently auto-applied (e.g. so the web frontend can let a player see their opening
/// hand before it gets played).
#[test]
fn test_auto_advance_forced_actions_false_pauses_before_forced_single_action() {
    let (deck_a, deck_b) = load_test_decks();
    let player_a: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_a });
    let player_b: Box<dyn Player> = Box::new(RandomPlayer { deck: deck_b });
    let mut game = Game::new(vec![player_a, player_b], 7);
    game.set_interactive(
        0,
        InteractiveConfig {
            auto_advance_forced_actions: false,
            ..Default::default()
        },
    );

    let mut state = game.get_state_clone();
    state.move_generation_stack.clear();
    state.current_player = 0;
    state.hands[0].clear();
    state.hands[0].push(get_card_by_enum(CardId::A1001Bulbasaur));
    // Player 1 also needs a placeable hand — clearing move_generation_stack above wipes both
    // players' queued opening-hand draws, so without this RandomPlayer would hit its own setup
    // turn with an empty hand and no legal actions at all.
    state.hands[1].clear();
    state.hands[1].push(get_card_by_enum(CardId::A1001Bulbasaur));
    game.set_state(state);

    match game.step() {
        PendingDecision::AwaitingAction { actor, actions } => {
            assert_eq!(actor, 0);
            assert_eq!(actions.len(), 1);
            assert!(matches!(actions[0].action, SimpleAction::Place(_, 0)));
        }
        other => panic!("expected a paused AwaitingAction for the forced Place, got {other:?}"),
    }
    assert!(
        game.get_state_clone().in_play_pokemon[0][0].is_none(),
        "the forced Place must not have been applied yet — it's still pending confirmation"
    );
}

/// Same setup, but with the default config (`auto_advance_forced_actions: true`) — the
/// placement should resolve straight through with no pause, matching behavior from before this
/// field existed.
#[test]
fn test_auto_advance_forced_actions_true_applies_forced_single_action_silently() {
    let (deck_a, deck_b) = load_test_decks();
    let player_a: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_a });
    let player_b: Box<dyn Player> = Box::new(RandomPlayer { deck: deck_b });
    let mut game = Game::new(vec![player_a, player_b], 7);
    game.set_interactive(0, InteractiveConfig::default());

    let mut state = game.get_state_clone();
    state.move_generation_stack.clear();
    state.current_player = 0;
    state.hands[0].clear();
    state.hands[0].push(get_card_by_enum(CardId::A1001Bulbasaur));
    // Player 1 also needs a placeable hand — clearing move_generation_stack above wipes both
    // players' queued opening-hand draws, so without this RandomPlayer would hit its own setup
    // turn with an empty hand and no legal actions at all.
    state.hands[1].clear();
    state.hands[1].push(get_card_by_enum(CardId::A1001Bulbasaur));
    game.set_state(state);

    game.step();
    assert!(
        game.get_state_clone().in_play_pokemon[0][0].is_some(),
        "Bulbasaur should have been placed automatically, with no pause"
    );
}

/// Attacking already ends your turn by the game's own rules — the EndTurn that follows must
/// auto-advance even with `auto_advance_forced_actions: false`, since re-confirming it would be
/// redundant (the player already made the turn-ending call when they chose to attack).
#[test]
fn test_end_turn_after_attacking_does_not_pause_for_confirmation() {
    let (deck_a, deck_b) = load_test_decks();
    let player_a: Box<dyn Player> = Box::new(InteractivePlayer { deck: deck_a });
    let player_b: Box<dyn Player> = Box::new(RandomPlayer { deck: deck_b });
    let mut game = Game::new(vec![player_a, player_b], 7);
    game.set_interactive(
        0,
        InteractiveConfig {
            auto_advance_forced_actions: false,
            ..Default::default()
        },
    );

    // Drive through real setup-phase placement for *both* players (not just player 0) — an
    // attack needs a real opposing active to hit, and forcing turn_count/current_player
    // directly without one leads to an inconsistent state the engine was never meant to see.
    let mut state = game.get_state_clone();
    state.move_generation_stack.clear();
    state.current_player = 0;
    state.hands[0].clear();
    state.hands[0].push(get_card_by_enum(CardId::A1001Bulbasaur));
    state.hands[1].clear();
    state.hands[1].push(get_card_by_enum(CardId::A1001Bulbasaur));
    game.set_state(state);

    // Setup phase needs an explicit EndTurn from each player after their Place, to signal
    // "done with my setup" — that's what actually advances to the other player (or, once both
    // are done, into turn 1) via forecast_end_turn, not the Place itself.
    let place0 = find_action(&game, |a| matches!(a.action, SimpleAction::Place(_, 0)));
    game.apply_action(&place0);
    let end0 = find_action(&game, |a| matches!(a.action, SimpleAction::EndTurn));
    game.apply_action(&end0);
    let place1 = find_action(&game, |a| matches!(a.action, SimpleAction::Place(_, 0)));
    game.apply_action(&place1);
    let end1 = find_action(&game, |a| matches!(a.action, SimpleAction::EndTurn));
    game.apply_action(&end1);
    // Both actives are now placed, which transitions turn_count 0 -> 1 and current_player to
    // whoever goes first (player 0, per forecast_end_turn's next_player calculation).
    assert_eq!(game.get_state_clone().turn_count, 1);
    assert_eq!(game.get_state_clone().current_player, 0);

    // Resolve the mandatory TurnStart draw so play_tick's move_generation_stack is clear and
    // Attack becomes available (it's queued ahead of everything else at the start of a turn).
    game.play_tick();

    // Attach plenty of energy directly (a public field) rather than resolving several real
    // turns' worth of energy-zone attachments — this test only cares about what happens once an
    // Attack has been made, not how the energy got there. Also clear the hand so nothing else
    // is legal after attacking.
    //
    // move_generation_stack must be cleared too: generate_possible_actions short-circuits to
    // its cached top frame when non-empty (see move_generation/mod.rs), which by this point
    // already holds the main-turn action list computed *before* these mutations (pushed when
    // play_tick resolved the draw above) — without clearing it, every subsequent
    // generate_possible_actions call keeps returning that stale, pre-mutation list no matter
    // what the state actually says.
    let mut state = game.get_state_clone();
    state.move_generation_stack.clear();
    state.in_play_pokemon[0][0]
        .as_mut()
        .unwrap()
        .attached_energy = vec![EnergyType::Grass; 4];
    state.hands[0].clear();
    game.set_state(state);

    let attack = find_action(&game, |a| matches!(a.action, SimpleAction::Attack(_)));
    game.apply_action(&attack);

    // Confirm we've actually reached the scenario the fix targets: EndTurn is now the sole
    // legal action for player 0, as a direct consequence of having just attacked.
    let (post_attack_actor, post_attack_actions) =
        game.get_state_clone().generate_possible_actions();
    assert_eq!(post_attack_actor, 0);
    assert_eq!(post_attack_actions.len(), 1);
    assert_eq!(post_attack_actions[0].action, SimpleAction::EndTurn);

    // `game.step()` resolves everything up to the *next* real pause — which, once the
    // post-attack EndTurn auto-advances as intended, is however far into player 1's
    // (non-interactive) turn or beyond that naturally reaches next. So rather than assert on
    // whatever that far-future decision happens to be, just confirm turn_count actually moved
    // past 1: that can only happen if the post-attack EndTurn was applied rather than sitting
    // paused waiting on a confirmation nobody sent.
    game.step();
    assert!(
        game.get_state_clone().turn_count > 1,
        "the post-attack EndTurn should have auto-applied, advancing past turn 1"
    );
}
