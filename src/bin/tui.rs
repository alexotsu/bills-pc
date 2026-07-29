use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use deckgym::{
    players::{fill_code_array, parse_player_code, PlayerCode},
    tui::{ui, App},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the first deck file
    deck_a: String,

    /// Path to the second deck file
    deck_b: String,

    /// Players' strategies as a comma-separated list
    #[arg(long, value_delimiter = ',', value_parser = parse_player_code)]
    players: Option<Vec<PlayerCode>>,

    /// Random seed for game simulation
    #[arg(long)]
    seed: Option<u64>,

    /// Let the human player (requires "h" in --players) choose which card to draw for their
    /// opening hand and each turn's draw, instead of drawing automatically.
    #[arg(long)]
    override_draws: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse CLI arguments
    let cli = Cli::parse();
    let player_codes = fill_code_array(cli.players);

    // Setup panic hook to restore terminal on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Attempt to restore terminal
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        // Call the original panic hook
        original_hook(panic_info);
    }));

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = App::new(
        &cli.deck_a,
        &cli.deck_b,
        player_codes,
        cli.seed,
        cli.override_draws,
    )?;
    let res = run_app(&mut terminal, app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}")
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(250);

    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        // Numeric keys for action selection (1-9)
                        KeyCode::Char(c @ '1'..='9') => {
                            let index = (c as usize) - ('1' as usize);
                            app.handle_action_selection(index);
                        }
                        // Replay mode controls
                        KeyCode::Down => app.next_state(),
                        KeyCode::Up => app.prev_state(),
                        KeyCode::Char('w') => app.jump_to_prev_turn(),
                        KeyCode::Char('s') => app.jump_to_next_turn(),
                        KeyCode::Left => app.scroll_player_hand_left(),
                        KeyCode::Right => app.scroll_player_hand_right(),
                        KeyCode::Char('c') => app.toggle_lock_actions_center(),
                        KeyCode::Char('A') => app.scroll_opponent_hand_left(),
                        KeyCode::Char('D') => app.scroll_opponent_hand_right(),
                        // Undo the last applied action (interactive mode only)
                        KeyCode::Char('u') | KeyCode::Backspace => app.undo(),
                        // Page through action/draw-choice lists longer than 9 items
                        KeyCode::PageDown => app.next_action_page(),
                        KeyCode::PageUp => app.prev_action_page(),
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            // Tick the game (advances game in interactive mode)
            app.tick_game();

            // Check if game is over
            if app.is_game_over() {
                return Ok(());
            }

            last_tick = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_ui_renders_without_panic() {
        // Create App using example decks
        let app = App::new(
            "example_decks/venusaur-exeggutor.txt",
            "example_decks/weezing-arbok.txt",
            vec![PlayerCode::R, PlayerCode::R],
            None,
            false,
        )
        .expect("Failed to create app");

        // Create a test backend
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        // This should not panic
        terminal.draw(|f| ui(f, &app)).expect("Failed to render UI");
    }

    /// Regression test for a real bug: when the human plays seat 0 (`--players h,r`), their
    /// own hand used to render as hidden "?" cards (hardcoded to always treat seat 0's hand as
    /// "the opponent's") while the bot's hand was fully revealed. Render to a `TestBackend` and
    /// scan the actual text buffer to confirm the human's real card names appear on screen and
    /// the bot's don't.
    #[test]
    fn test_seat_0_human_sees_own_hand_not_opponents() {
        let mut app = App::new(
            "example_decks/venusaur-exeggutor.txt",
            "example_decks/weezing-arbok.txt",
            vec![PlayerCode::H, PlayerCode::R],
            Some(7),
            false,
        )
        .expect("Failed to create app");

        // Drive forward until both actives are placed (turn >= 1), auto-picking the first
        // option for every real human decision along the way.
        for _ in 0..500 {
            if app.get_state().turn_count >= 1 {
                break;
            }
            let possible_actions = app.get_possible_actions();
            if app.is_current_actor_human() && possible_actions.len() > 1 {
                app.handle_action_selection(0);
            }
            app.tick_game();
        }
        assert_eq!(
            app.bottom_seat(),
            0,
            "seat 0 should render at the bottom when it's the only human seat"
        );

        let backend = TestBackend::new(140, 50);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");
        terminal.draw(|f| ui(f, &app)).expect("Failed to render UI");

        let mut screen = String::new();
        let buffer = terminal.backend().buffer();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                screen.push_str(buffer[(x, y)].symbol());
            }
            screen.push('\n');
        }

        let human_hand = &app.get_state().hands[0];
        assert!(
            !human_hand.is_empty(),
            "test setup: the human seat should have drawn its opening hand by turn 1"
        );
        for card in human_hand {
            assert!(
                screen.contains(&card.get_name()),
                "human's own card {} should be visible on screen",
                card.get_name()
            );
        }

        // Card names already placed on the bot's board (active/bench) legitimately appear on
        // screen regardless of hand concealment — only check hand cards that aren't also an
        // in-play Pokemon name, to avoid a false positive from e.g. a second copy of a
        // basic that's already active.
        let bot_state = app.get_state();
        let bot_board_names: Vec<String> = bot_state.in_play_pokemon[1]
            .iter()
            .flatten()
            .map(|p| p.card.get_name())
            .collect();
        let bot_hand = &bot_state.hands[1];
        assert!(
            !bot_hand.is_empty(),
            "test setup: the bot seat should have drawn its opening hand by turn 1"
        );
        for card in bot_hand
            .iter()
            .filter(|c| !bot_board_names.contains(&c.get_name()))
        {
            assert!(
                !screen.contains(&card.get_name()),
                "bot's concealed card {} should not be visible on screen",
                card.get_name()
            );
        }

        assert!(screen.contains("P1 Active"));
        assert!(screen.contains("P2 Active"));
    }
}
