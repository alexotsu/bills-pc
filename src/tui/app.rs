use crate::{
    actions::{Action, SimpleAction},
    models::Card,
    players::{create_players, Player, PlayerCode},
    Deck, Game, State,
};
use rand::{thread_rng, Rng};
use std::error::Error;

/// How many items (actions or draw candidates) are shown, and selectable via keys 1-9, on one
/// page. Lists longer than this are paged through with PageUp/PageDown rather than truncated.
pub(crate) const ITEMS_PER_PAGE: usize = 9;

pub enum AppMode {
    Replay {
        states: Vec<State>,
        actions: Vec<Action>,
        current_index: usize,
    },
    Interactive {
        game: Box<Game<'static>>,
        current_actor: usize,
        possible_actions: Vec<Action>,
        action_history: Vec<Action>, // Track actions as they happen
        turn_history: Vec<u8>,       // Track turn number when each action was taken
        // Full state snapshot taken immediately *before* each entry in `action_history` was
        // applied (same length, same indices). Powers `App::undo`: popping the last snapshot
        // and restoring it rolls the game back one action at a time — human or bot — so a
        // misclick can be undone by pressing undo until you're back before it.
        state_history: Vec<State>,
        // Which seats are human-controlled (`--players` code `h`), derived once at
        // construction. Any number of seats can be human — including both, for local
        // hot-seat play. A seat that isn't human auto-plays through its own `Player`
        // exactly like bulk simulation; a human seat always pauses for a click.
        human_seats: [bool; 2],
        // If true, human seats' forced single-card draws (opening hand + turn-start) pause
        // for a card pick instead of resolving automatically.
        draw_override_enabled: bool,
        // Some(candidates) when the current human seat must pick which card to draw next.
        draw_choice: Option<Vec<Card>>,
    },
}

pub enum SelectionState {
    AwaitingActionSelection,
    ActionSelected { action_index: usize },
}

pub struct App {
    pub mode: AppMode,
    pub selection_state: SelectionState,
    pub scroll_offset: u16,
    pub player_hand_scroll: usize,
    pub opponent_hand_scroll: usize,
    pub lock_actions_center: bool,
    // Which page of the current action/draw-choice list is shown (see `ITEMS_PER_PAGE`).
    // Reset to 0 whenever a new list is presented (a new decision, or after undo).
    pub action_page: usize,
}

fn action_priority_for_tui(action: &SimpleAction) -> u8 {
    match action {
        SimpleAction::Place(_, _) => 0,
        SimpleAction::Evolve { .. } => 1,
        SimpleAction::Play { .. } => 2,
        SimpleAction::Attach { .. }
        | SimpleAction::AttachFromDiscard { .. }
        | SimpleAction::AttachTool { .. } => 3,
        SimpleAction::Attack(_) => 4,
        SimpleAction::Retreat(_) => 5,
        SimpleAction::EndTurn => 255,
        _ => 6,
    }
}

fn sort_actions_for_tui(actions: &mut Vec<Action>) {
    let mut indexed_actions: Vec<(usize, Action)> = actions.drain(..).enumerate().collect();
    indexed_actions.sort_by_key(|(idx, action)| (action_priority_for_tui(&action.action), *idx));
    *actions = indexed_actions
        .into_iter()
        .map(|(_, action)| action)
        .collect();
}

/// If `actor`'s only legal move is a forced single-card draw and draw override is enabled,
/// returns the candidate cards to offer instead of letting it auto-resolve. `actor_is_human`
/// gates this: a non-human (bot) seat's draws always auto-resolve regardless of the flag. All
/// remaining deck cards are returned (not capped to one page) — `ITEMS_PER_PAGE`-based paging
/// (PageUp/PageDown) handles showing/selecting from lists longer than one page.
fn maybe_offer_draw_choice(
    game: &Game,
    actor: usize,
    actor_is_human: bool,
    possible_actions: &[Action],
    draw_override_enabled: bool,
) -> Option<Vec<Card>> {
    if !actor_is_human || !draw_override_enabled {
        return None;
    }
    let [action] = possible_actions else {
        return None;
    };
    if !matches!(action.action, SimpleAction::DrawCard { .. }) {
        return None;
    }
    let state = game.get_state_clone();
    if state.decks[actor].cards.is_empty() {
        // Nothing to choose between; let it auto-resolve (a no-op draw) instead of showing an
        // empty picker.
        return None;
    }
    Some(state.decks[actor].cards.clone())
}

impl App {
    pub fn new(
        deck_a_path: &str,
        deck_b_path: &str,
        player_codes: Vec<PlayerCode>,
        seed: Option<u64>,
        draw_override_enabled: bool,
    ) -> Result<App, Box<dyn Error>> {
        // Load decks from files
        let deck_a = Deck::from_file(deck_a_path)?;
        let deck_b = Deck::from_file(deck_b_path)?;

        // Detect which seats (if any) are human-controlled. Any number of seats can be human
        // (including both, for local hot-seat play) — a seat auto-plays through its own
        // `Player` unless its code is `h`.
        let human_seats = [
            player_codes.first() == Some(&PlayerCode::H),
            player_codes.get(1) == Some(&PlayerCode::H),
        ];
        let has_human = human_seats.contains(&true);

        // Use provided seed or generate a random one
        let seed = seed.unwrap_or_else(|| {
            let mut rng = thread_rng();
            rng.gen::<u64>()
        });

        let mode = if has_human {
            // Interactive mode - create live game.
            let players: Vec<Box<dyn Player>> = create_players(deck_a, deck_b, player_codes);
            let game = Box::new(Game::new(players, seed));

            // Get initial state and possible actions
            let (current_actor, mut possible_actions) =
                game.get_state_clone().generate_possible_actions();
            sort_actions_for_tui(&mut possible_actions);
            let draw_choice = maybe_offer_draw_choice(
                &game,
                current_actor,
                human_seats[current_actor],
                &possible_actions,
                draw_override_enabled,
            );

            AppMode::Interactive {
                game,
                current_actor,
                possible_actions,
                action_history: vec![],
                turn_history: vec![],
                state_history: vec![],
                human_seats,
                draw_override_enabled,
                draw_choice,
            }
        } else {
            // Replay mode - pre-compute entire game
            let players: Vec<Box<dyn Player>> = create_players(deck_a, deck_b, player_codes);
            let mut game = Game::new(players, seed);

            let mut states = Vec::new();
            let mut actions = Vec::new();
            states.push(game.get_state_clone());

            while !game.is_game_over() {
                let action = game.play_tick();
                actions.push(action);
                states.push(game.get_state_clone());
            }

            AppMode::Replay {
                states,
                actions,
                current_index: 0,
            }
        };

        Ok(App {
            mode,
            selection_state: SelectionState::AwaitingActionSelection,
            scroll_offset: 0,
            player_hand_scroll: 0,
            opponent_hand_scroll: 0,
            lock_actions_center: true,
            action_page: 0,
        })
    }

    pub fn get_state(&self) -> State {
        match &self.mode {
            AppMode::Replay {
                states,
                current_index,
                ..
            } => states[*current_index].clone(),
            AppMode::Interactive { game, .. } => game.get_state_clone(),
        }
    }

    // Helper method to calculate turn boundaries in the battle log
    // Returns the scroll offset (line number) where each turn header appears
    fn calculate_turn_boundaries(&self) -> Vec<usize> {
        let mut boundaries = Vec::new();
        let mut line_count = 0;

        match &self.mode {
            AppMode::Interactive {
                action_history,
                turn_history,
                ..
            } => {
                // Even if there are no recorded actions yet, we should at least
                // expose the initial turn header so "jump" can move the battle
                // log to the start of a turn in interactive mode.
                let mut current_turn: u8 = if !turn_history.is_empty() {
                    turn_history[0]
                } else {
                    // No actions yet - use the game's current turn number as the initial header
                    self.get_state().turn_count
                };

                // Initial turn header
                boundaries.push(line_count);
                line_count += 1;

                // For each recorded action add its line and detect turn changes
                for i in 0..action_history.len() {
                    // Each action occupies a single line
                    line_count += 1;

                    // If next action has different turn, add header boundary
                    if i + 1 < turn_history.len() {
                        let next_turn = turn_history[i + 1];
                        if next_turn != current_turn {
                            line_count += 1; // empty line before header
                            boundaries.push(line_count);
                            line_count += 1; // header line
                            current_turn = next_turn;
                        }
                    }
                }
            }
            AppMode::Replay {
                states,
                actions,
                current_index,
                ..
            } => {
                if states.is_empty() {
                    return boundaries;
                }

                let mut current_turn = states[0].turn_count;
                boundaries.push(line_count); // Initial turn header
                line_count += 1;

                for i in 0..actions.len() {
                    // Add cursor marker line if this is the current action
                    if i == *current_index && i < actions.len() {
                        line_count += 1; // Cursor marker ">>> CURRENT <<<"
                    }

                    // Each action takes exactly 1 line
                    line_count += 1;

                    // Check if turn changed after this action
                    if i + 1 < states.len() {
                        let next_turn = states[i + 1].turn_count;
                        if next_turn != current_turn && i + 1 < actions.len() {
                            line_count += 1; // Empty line
                            boundaries.push(line_count);
                            line_count += 1; // Turn header
                            current_turn = next_turn;
                        }
                    }
                }
            }
        }

        boundaries
    }

    pub fn next_state(&mut self) {
        if let AppMode::Replay {
            current_index,
            states,
            ..
        } = &mut self.mode
        {
            if *current_index < states.len() - 1 {
                *current_index += 1;
            }
        }
    }

    pub fn prev_state(&mut self) {
        if let AppMode::Replay { current_index, .. } = &mut self.mode {
            if *current_index > 0 {
                *current_index -= 1;
            }
        }
    }

    pub fn toggle_lock_actions_center(&mut self) {
        self.lock_actions_center = !self.lock_actions_center;
    }

    fn jump_turn(&mut self, forward: bool) {
        if self.lock_actions_center {
            // Center lock on: jump state to beginning of next/previous turn
            match &mut self.mode {
                AppMode::Replay {
                    states,
                    current_index,
                    ..
                } => {
                    let valid_range = if forward {
                        *current_index < states.len()
                    } else {
                        *current_index > 0
                    };

                    if valid_range {
                        let current_turn = states[*current_index].turn_count;

                        // Find a state with different turn number
                        let mut target_turn = None;
                        if forward {
                            for state in states.iter().skip(*current_index + 1) {
                                if state.turn_count != current_turn {
                                    target_turn = Some(state.turn_count);
                                    break;
                                }
                            }
                        } else {
                            for state in states.iter().take(*current_index).rev() {
                                if state.turn_count != current_turn {
                                    target_turn = Some(state.turn_count);
                                    break;
                                }
                            }
                        }

                        // If we found a different turn, find the FIRST state of that turn
                        if let Some(turn) = target_turn {
                            for (i, state) in states.iter().enumerate() {
                                if state.turn_count == turn {
                                    *current_index = i;
                                    return;
                                }
                            }
                        }
                    }
                }
                AppMode::Interactive { .. } => {
                    // In interactive mode we don't have a precomputed states vector,
                    // but we can still move the battle log view to the next/previous
                    // turn header. Compute turn boundaries and adjust the scroll
                    // offset similarly to the non-center-lock branch.
                    let boundaries = self.calculate_turn_boundaries();
                    if boundaries.is_empty() {
                        return;
                    }

                    let current_line = self.scroll_offset as usize;
                    if forward {
                        if let Some(&next_line) =
                            boundaries.iter().find(|&&line| line > current_line)
                        {
                            self.scroll_offset = next_line as u16;
                        }
                    } else if let Some(&prev_line) =
                        boundaries.iter().rev().find(|&&line| line < current_line)
                    {
                        self.scroll_offset = prev_line as u16;
                    }
                }
            }
        } else {
            // Center lock off: just scroll the battle log to next/previous turn header
            let boundaries = self.calculate_turn_boundaries();
            let current_line = self.scroll_offset as usize;

            if forward {
                if let Some(&next_line) = boundaries.iter().find(|&&line| line > current_line) {
                    self.scroll_offset = next_line as u16;
                }
            } else if let Some(&prev_line) =
                boundaries.iter().rev().find(|&&line| line < current_line)
            {
                self.scroll_offset = prev_line as u16;
            }
        }
    }

    pub fn jump_to_next_turn(&mut self) {
        self.jump_turn(true);
    }

    pub fn jump_to_prev_turn(&mut self) {
        self.jump_turn(false);
    }

    pub fn scroll_page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(10);
    }

    pub fn scroll_page_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(10);
    }

    pub fn scroll_player_hand_left(&mut self) {
        self.player_hand_scroll = self.player_hand_scroll.saturating_sub(1);
    }

    pub fn scroll_player_hand_right(&mut self) {
        let bottom_seat = self.bottom_seat();
        let player_hand_size = self.get_state().hands[bottom_seat].len();
        if self.player_hand_scroll < player_hand_size.saturating_sub(5) {
            self.player_hand_scroll += 1;
        }
    }

    pub fn scroll_opponent_hand_left(&mut self) {
        self.opponent_hand_scroll = self.opponent_hand_scroll.saturating_sub(1);
    }

    pub fn scroll_opponent_hand_right(&mut self) {
        let top_seat = 1 - self.bottom_seat();
        let opponent_hand_size = self.get_state().hands[top_seat].len();
        if self.opponent_hand_scroll < opponent_hand_size.saturating_sub(5) {
            self.opponent_hand_scroll += 1;
        }
    }

    // Interactive mode methods
    /// `index` is a 0-based position *within the current page* (i.e. what key "1"-"9" means on
    /// screen); it's translated to an absolute index into the full list before being recorded.
    pub fn handle_action_selection(&mut self, index: usize) {
        if let AppMode::Interactive {
            possible_actions,
            draw_choice,
            ..
        } = &self.mode
        {
            let count = draw_choice
                .as_ref()
                .map_or(possible_actions.len(), |candidates| candidates.len());
            let absolute_index = self.action_page * ITEMS_PER_PAGE + index;
            if absolute_index < count {
                self.selection_state = SelectionState::ActionSelected {
                    action_index: absolute_index,
                };
            }
        }
    }

    /// Total number of items in whichever list (actions or draw candidates) is currently up
    /// for selection. 0 outside of a human's turn.
    fn current_selectable_count(&self) -> usize {
        match &self.mode {
            AppMode::Interactive {
                possible_actions,
                draw_choice,
                ..
            } => draw_choice
                .as_ref()
                .map_or(possible_actions.len(), |candidates| candidates.len()),
            AppMode::Replay { .. } => 0,
        }
    }

    /// Moves to the next page of the current action/draw-choice list, if there is one. See
    /// `ITEMS_PER_PAGE`.
    pub fn next_action_page(&mut self) {
        let total = self.current_selectable_count();
        let max_page = total.saturating_sub(1) / ITEMS_PER_PAGE;
        if self.action_page < max_page {
            self.action_page += 1;
        }
    }

    pub fn prev_action_page(&mut self) {
        self.action_page = self.action_page.saturating_sub(1);
    }

    pub fn tick_game(&mut self) {
        if let AppMode::Interactive {
            game,
            current_actor,
            possible_actions,
            draw_choice,
            human_seats,
            draw_override_enabled,
            action_history,
            turn_history,
            state_history,
        } = &mut self.mode
        {
            match &self.selection_state {
                SelectionState::ActionSelected { action_index } => {
                    let state_before = game.get_state_clone();
                    let current_turn = state_before.turn_count;

                    if let Some(candidates) = draw_choice.take() {
                        // Resolve the deciding seat's draw pick: reorder its deck so the
                        // chosen card is drawn next, then apply the already-queued DrawCard
                        // action normally (hand-cap check and all run completely unmodified).
                        let card = candidates[*action_index].clone();
                        let draw_action = possible_actions[0].clone();
                        let mut state = state_before.clone();
                        state
                            .move_card_to_front_of_deck(*current_actor, &card)
                            .expect("a listed candidate should always be in the deck");
                        game.set_state(state);
                        action_history.push(draw_action.clone());
                        turn_history.push(current_turn);
                        state_history.push(state_before);
                        game.apply_action(&draw_action);
                    } else {
                        let action = possible_actions[*action_index].clone();
                        action_history.push(action.clone());
                        turn_history.push(current_turn);
                        state_history.push(state_before);
                        game.apply_action(&action);
                    }

                    // Reset selection state
                    self.selection_state = SelectionState::AwaitingActionSelection;
                    self.action_page = 0;

                    // Refresh game state and possible actions for next turn
                    let (new_actor, mut new_actions) =
                        game.get_state_clone().generate_possible_actions();
                    sort_actions_for_tui(&mut new_actions);
                    *current_actor = new_actor;
                    *possible_actions = new_actions;
                    *draw_choice = None;
                }
                SelectionState::AwaitingActionSelection => {
                    if !human_seats[*current_actor] {
                        // Record current turn before the bot plays
                        let state_before = game.get_state_clone();
                        let current_turn = state_before.turn_count;

                        // Non-human seat: driven by its own configured Player, exactly like
                        // bulk simulation.
                        let action = game.play_tick();
                        action_history.push(action);
                        turn_history.push(current_turn);
                        state_history.push(state_before);

                        // Refresh for next turn
                        let (new_actor, mut new_actions) =
                            game.get_state_clone().generate_possible_actions();
                        sort_actions_for_tui(&mut new_actions);
                        *current_actor = new_actor;
                        *possible_actions = new_actions;
                        self.action_page = 0;
                    } else if draw_choice.is_none() {
                        if let Some(candidates) = maybe_offer_draw_choice(
                            game,
                            *current_actor,
                            human_seats[*current_actor],
                            possible_actions,
                            *draw_override_enabled,
                        ) {
                            // Offer this seat's human a card pick instead of auto-resolving.
                            *draw_choice = Some(candidates);
                            self.action_page = 0;
                        } else if possible_actions.len() == 1 {
                            // Forced single action (non-draw, or draw override disabled):
                            // auto-apply, matching HumanPlayer's own "only one option, just
                            // take it" convention.
                            let state_before = game.get_state_clone();
                            let current_turn = state_before.turn_count;
                            let action = possible_actions[0].clone();
                            action_history.push(action.clone());
                            turn_history.push(current_turn);
                            state_history.push(state_before);
                            game.apply_action(&action);

                            let (new_actor, mut new_actions) =
                                game.get_state_clone().generate_possible_actions();
                            sort_actions_for_tui(&mut new_actions);
                            *current_actor = new_actor;
                            *possible_actions = new_actions;
                            self.action_page = 0;
                        }
                        // else: multiple choices, wait for human input
                    }
                    // else: already offering a draw choice, wait for human input
                }
            }
        }
    }

    /// Whether there's a recorded action to roll back to. Always false in Replay mode (use
    /// Up/Down to navigate a replay instead) or before anything has happened yet.
    pub fn can_undo(&self) -> bool {
        match &self.mode {
            AppMode::Replay { .. } => false,
            AppMode::Interactive { state_history, .. } => !state_history.is_empty(),
        }
    }

    /// Rolls the live game back to the state immediately before the last applied action —
    /// human or bot, whichever happened most recently — undoing exactly one step. Press
    /// repeatedly to walk further back, e.g. past an opponent's move that landed in between a
    /// misclick and the undo key press. A no-op if `can_undo()` is false.
    pub fn undo(&mut self) {
        if let AppMode::Interactive {
            game,
            current_actor,
            possible_actions,
            draw_choice,
            human_seats,
            draw_override_enabled,
            action_history,
            turn_history,
            state_history,
        } = &mut self.mode
        {
            let Some(previous_state) = state_history.pop() else {
                return;
            };
            action_history.pop();
            turn_history.pop();
            game.set_state(previous_state);
            self.selection_state = SelectionState::AwaitingActionSelection;
            self.action_page = 0;

            let (new_actor, mut new_actions) = game.get_state_clone().generate_possible_actions();
            sort_actions_for_tui(&mut new_actions);
            *draw_choice = maybe_offer_draw_choice(
                game,
                new_actor,
                human_seats[new_actor],
                &new_actions,
                *draw_override_enabled,
            );
            *current_actor = new_actor;
            *possible_actions = new_actions;
        }
    }

    pub fn is_game_over(&self) -> bool {
        match &self.mode {
            AppMode::Replay { .. } => false, // Replay never "ends" automatically
            AppMode::Interactive { game, .. } => game.is_game_over(),
        }
    }

    pub fn get_possible_actions(&self) -> Vec<Action> {
        match &self.mode {
            AppMode::Replay {
                states,
                current_index,
                ..
            } => {
                let mut actions = states[*current_index].generate_possible_actions().1;
                sort_actions_for_tui(&mut actions);
                actions
            }
            AppMode::Interactive {
                possible_actions, ..
            } => possible_actions.clone(),
        }
    }

    /// Some(candidates) when the human must currently pick which card to draw next (see
    /// `--override-draws`). Never populated in Replay mode.
    pub fn get_draw_choice(&self) -> Option<&[Card]> {
        match &self.mode {
            AppMode::Replay { .. } => None,
            AppMode::Interactive { draw_choice, .. } => draw_choice.as_deref(),
        }
    }

    pub fn get_current_actor(&self) -> usize {
        match &self.mode {
            AppMode::Replay {
                states,
                current_index,
                ..
            } => states[*current_index].generate_possible_actions().0,
            AppMode::Interactive { current_actor, .. } => *current_actor,
        }
    }

    /// Whether the seat currently deciding is human-controlled (i.e. should wait for a key
    /// press rather than auto-play). Always false in Replay mode.
    pub fn is_current_actor_human(&self) -> bool {
        match &self.mode {
            AppMode::Replay { .. } => false,
            AppMode::Interactive {
                current_actor,
                human_seats,
                ..
            } => human_seats[*current_actor],
        }
    }

    /// The seat rendered at the bottom of the board (the "your side" position, nearest the
    /// hand panel you click cards from). If exactly one seat is human, that seat is always
    /// shown at the bottom regardless of whether it's seat 0 or seat 1 — so your own board is
    /// always in the same place whether you're `--players h,r` or `--players r,h`. Falls back
    /// to seat 1 when both or neither seat is human (hot-seat play, or Replay mode), matching
    /// this TUI's original fixed layout.
    pub fn bottom_seat(&self) -> usize {
        match &self.mode {
            AppMode::Replay { .. } => 1,
            AppMode::Interactive { human_seats, .. } => {
                if human_seats[0] && !human_seats[1] {
                    0
                } else {
                    1
                }
            }
        }
    }

    /// Whether `seat`'s hand should be rendered face-up. Human seats are always revealed (it's
    /// your own hand); in Replay mode only the bottom seat is revealed, matching this TUI's
    /// original single-perspective convention.
    pub fn is_hand_revealed(&self, seat: usize) -> bool {
        match &self.mode {
            AppMode::Replay { .. } => seat == self.bottom_seat(),
            AppMode::Interactive { human_seats, .. } => human_seats[seat],
        }
    }

    pub fn get_current_state_index(&self) -> usize {
        match &self.mode {
            AppMode::Replay { current_index, .. } => *current_index,
            AppMode::Interactive { .. } => 0, // Not really meaningful in interactive mode
        }
    }

    pub fn get_states_len(&self) -> usize {
        match &self.mode {
            AppMode::Replay { states, .. } => states.len(),
            AppMode::Interactive { .. } => 1, // Only current state
        }
    }

    pub fn get_actions(&self) -> Vec<Action> {
        match &self.mode {
            AppMode::Replay { actions, .. } => actions.clone(),
            AppMode::Interactive { action_history, .. } => action_history.clone(),
        }
    }

    pub fn get_turn_history(&self) -> Option<Vec<u8>> {
        match &self.mode {
            AppMode::Interactive { turn_history, .. } => Some(turn_history.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{sort_actions_for_tui, App, AppMode, SelectionState, ITEMS_PER_PAGE};
    use crate::{
        actions::{Action, SimpleAction},
        models::{Attack, Card, EnergyType, PokemonCard, TrainerCard, TrainerType},
        players::PlayerCode,
    };

    fn action(action: SimpleAction) -> Action {
        Action {
            actor: 1,
            action,
            is_stack: false,
        }
    }

    fn test_pokemon(name: &str) -> Card {
        Card::Pokemon(PokemonCard {
            id: format!("test-{name}"),
            name: name.to_string(),
            stage: 0,
            evolves_from: None,
            hp: 60,
            energy_type: EnergyType::Colorless,
            ability: None,
            attacks: vec![],
            weakness: None,
            retreat_cost: vec![],
            rarity: String::new(),
            booster_pack: String::new(),
        })
    }

    #[test]
    fn sorts_actions_for_tui_in_expected_priority_order() {
        let mut actions = vec![
            action(SimpleAction::EndTurn),
            action(SimpleAction::Retreat(1)),
            action(SimpleAction::Attack(Attack {
                energy_required: vec![],
                title: "Test Attack".to_string(),
                fixed_damage: 0,
                effect: None,
            })),
            action(SimpleAction::Attach {
                attachments: vec![],
                is_turn_energy: true,
            }),
            action(SimpleAction::Play {
                trainer_card: TrainerCard {
                    id: "potion".to_string(),
                    trainer_card_type: TrainerType::Item,
                    name: "Potion".to_string(),
                    effect: String::new(),
                    rarity: String::new(),
                    booster_pack: String::new(),
                },
            }),
            action(SimpleAction::Evolve {
                evolution: test_pokemon("Ivysaur"),
                in_play_idx: 0,
                from_deck: false,
            }),
            action(SimpleAction::Place(test_pokemon("Bulbasaur"), 1)),
        ];

        sort_actions_for_tui(&mut actions);

        assert!(matches!(actions[0].action, SimpleAction::Place(_, _)));
        assert!(matches!(actions[1].action, SimpleAction::Evolve { .. }));
        assert!(matches!(actions[2].action, SimpleAction::Play { .. }));
        assert!(matches!(actions[3].action, SimpleAction::Attach { .. }));
        assert!(matches!(actions[4].action, SimpleAction::Attack(_)));
        assert!(matches!(actions[5].action, SimpleAction::Retreat(_)));
        assert!(matches!(actions[6].action, SimpleAction::EndTurn));
    }

    /// Headless functional test for `--override-draws`: drives a live `App` purely through its
    /// public methods (`tick_game`/`handle_action_selection`), the same calls the real key-press
    /// loop in `src/bin/tui.rs` makes, and confirms the human seat gets offered at least one
    /// draw choice and that picking one resolves it.
    #[test]
    fn test_draw_override_flow_lets_human_pick_a_card_to_draw() {
        let mut app = App::new(
            "example_decks/venusaur-exeggutor.txt",
            "example_decks/weezing-arbok.txt",
            vec![PlayerCode::R, PlayerCode::H],
            Some(7),
            true, // override_draws
        )
        .expect("App::new should succeed");

        let mut saw_draw_choice = false;
        let mut resolved_a_draw_choice = false;

        for _ in 0..2_000 {
            if app.is_game_over() {
                break;
            }

            if let Some(candidates) = app.get_draw_choice() {
                assert!(
                    !candidates.is_empty(),
                    "draw choice should offer at least one candidate"
                );
                saw_draw_choice = true;
                app.handle_action_selection(0);
                app.tick_game();
                if app.get_draw_choice().is_none() {
                    resolved_a_draw_choice = true;
                }
                continue;
            }

            // Only click when there's a genuine choice: a single legal action is left alone
            // so `tick_game` gets to auto-resolve it (or, for a human draw, offer a card pick)
            // on its own — exactly like the real key-press loop, where no key press happens
            // for forced single-choice steps.
            let possible_actions = app.get_possible_actions();
            if app.is_current_actor_human() && possible_actions.len() > 1 {
                app.handle_action_selection(0);
            }
            app.tick_game();
        }

        assert!(
            saw_draw_choice,
            "the human seat should have been offered at least one draw choice"
        );
        assert!(
            resolved_a_draw_choice,
            "picking a draw candidate should resolve the draw and clear draw_choice"
        );
    }

    /// Without `--override-draws`, the human seat's draws must keep resolving automatically
    /// (matching pre-existing behavior) instead of ever pausing for a card pick.
    #[test]
    fn test_draw_choice_never_offered_when_override_disabled() {
        let mut app = App::new(
            "example_decks/venusaur-exeggutor.txt",
            "example_decks/weezing-arbok.txt",
            vec![PlayerCode::R, PlayerCode::H],
            Some(7),
            false, // override_draws
        )
        .expect("App::new should succeed");

        for _ in 0..2_000 {
            if app.is_game_over() {
                break;
            }
            assert!(
                app.get_draw_choice().is_none(),
                "draw choice should never be offered when override_draws is disabled"
            );

            // Only click when there's a genuine choice: a single legal action is left alone
            // so `tick_game` gets to auto-resolve it (or, for a human draw, offer a card pick)
            // on its own — exactly like the real key-press loop, where no key press happens
            // for forced single-choice steps.
            let possible_actions = app.get_possible_actions();
            if app.is_current_actor_human() && possible_actions.len() > 1 {
                app.handle_action_selection(0);
            }
            app.tick_game();
        }
    }

    /// Sanity check that `AppMode::Interactive` is actually what `App::new` produces when a
    /// human player code is present (the mode `tick_game`/draw override logic requires).
    #[test]
    fn test_human_player_code_selects_interactive_mode() {
        let app = App::new(
            "example_decks/venusaur-exeggutor.txt",
            "example_decks/weezing-arbok.txt",
            vec![PlayerCode::R, PlayerCode::H],
            Some(7),
            true,
        )
        .expect("App::new should succeed");

        assert!(matches!(app.mode, AppMode::Interactive { .. }));
    }

    /// `--players h,h` (both seats human) must be drivable end-to-end purely through clicks —
    /// no seat should ever fall through to a bot `Player::decision_fn` (which for `h` is
    /// `HumanPlayer`, whose `decision_fn` blocks on stdin and would hang a real TUI session).
    #[test]
    fn test_both_seats_human_plays_via_clicks_only() {
        let mut app = App::new(
            "example_decks/venusaur-exeggutor.txt",
            "example_decks/weezing-arbok.txt",
            vec![PlayerCode::H, PlayerCode::H],
            Some(7),
            true, // override_draws
        )
        .expect("App::new should succeed");

        let mut saw_actor_0_turn = false;
        let mut saw_actor_1_turn = false;

        for _ in 0..2_000 {
            if app.is_game_over() {
                break;
            }
            assert!(
                app.is_current_actor_human(),
                "every decision should belong to a human seat when both players are `h`"
            );
            match app.get_current_actor() {
                0 => saw_actor_0_turn = true,
                1 => saw_actor_1_turn = true,
                other => panic!("unexpected actor {other}"),
            }

            if let Some(candidates) = app.get_draw_choice() {
                if !candidates.is_empty() {
                    app.handle_action_selection(0);
                }
                app.tick_game();
                continue;
            }

            let possible_actions = app.get_possible_actions();
            if possible_actions.len() > 1 {
                app.handle_action_selection(0);
            }
            app.tick_game();
        }

        assert!(
            saw_actor_0_turn,
            "seat 0 should have taken at least one turn"
        );
        assert!(
            saw_actor_1_turn,
            "seat 1 should have taken at least one turn"
        );
    }

    #[test]
    fn test_undo_reverts_state_history_and_action_history_together() {
        let mut app = App::new(
            "example_decks/venusaur-exeggutor.txt",
            "example_decks/weezing-arbok.txt",
            vec![PlayerCode::R, PlayerCode::H],
            Some(7),
            false,
        )
        .expect("App::new should succeed");

        assert!(
            !app.can_undo(),
            "nothing has happened yet, there should be nothing to undo"
        );

        // Advance a few steps (mix of bot auto-play and human single-choice auto-applies).
        for _ in 0..5 {
            app.tick_game();
        }
        assert!(app.can_undo(), "several actions have been applied by now");

        let state_before_undo = app.get_state();
        let actions_before_undo = app.get_actions();
        assert!(!actions_before_undo.is_empty());

        app.undo();

        let state_after_undo = app.get_state();
        let actions_after_undo = app.get_actions();
        assert_eq!(
            actions_after_undo.len(),
            actions_before_undo.len() - 1,
            "undo should remove exactly the last recorded action"
        );
        assert_eq!(
            actions_after_undo,
            actions_before_undo[..actions_before_undo.len() - 1],
            "remaining action history should be an unmodified prefix of the original"
        );
        assert_ne!(
            state_after_undo, state_before_undo,
            "the game state should have actually changed"
        );

        // Undoing repeatedly should keep unwinding, then become a no-op once history is empty.
        while app.can_undo() {
            app.undo();
        }
        assert!(app.get_actions().is_empty());
        let state_at_start = app.get_state();
        app.undo(); // no-op: nothing left to undo
        assert_eq!(
            app.get_state(),
            state_at_start,
            "undo with empty history should not change anything"
        );
    }

    /// Regression test: lists longer than 9 items (a fresh 20-card deck's draw candidates,
    /// well before any pagination existed) used to be silently truncated with no way to reach
    /// items past #9. Confirms paging changes which absolute item key "1"-"9" selects.
    #[test]
    fn test_draw_choice_paging_reaches_items_past_page_one() {
        let mut app = App::new(
            "example_decks/venusaur-exeggutor.txt",
            "example_decks/weezing-arbok.txt",
            vec![PlayerCode::R, PlayerCode::H],
            Some(7),
            true, // override_draws
        )
        .expect("App::new should succeed");

        // Drive to the first draw choice offered to the human (their first InitialHand draw).
        for _ in 0..100 {
            if app.get_draw_choice().is_some() {
                break;
            }
            app.tick_game();
        }
        let candidates = app
            .get_draw_choice()
            .expect("test setup: should have reached a draw choice")
            .to_vec();
        assert!(
            candidates.len() > ITEMS_PER_PAGE,
            "test setup: a fresh 20-card deck should offer more than one page of candidates"
        );
        assert_eq!(app.action_page, 0, "should start on the first page");

        app.prev_action_page();
        assert_eq!(
            app.action_page, 0,
            "paging before the first page should be a no-op"
        );

        app.next_action_page();
        assert_eq!(
            app.action_page, 1,
            "should have advanced to the second page"
        );

        // Key "3" on page 2 should resolve to absolute index 1*9 + 2 = 11, not 2.
        app.handle_action_selection(2);
        assert!(matches!(
            app.selection_state,
            SelectionState::ActionSelected { action_index: 11 }
        ));

        app.tick_game();
        let expected_card = &candidates[11];
        assert!(
            app.get_state().hands[1].contains(expected_card),
            "the card at absolute index 11 (page 2, key 3) should have been drawn"
        );

        // The page resets once the choice is resolved and a new decision is presented.
        assert_eq!(
            app.action_page, 0,
            "action_page should reset back to 0 for the next decision"
        );
    }
}
