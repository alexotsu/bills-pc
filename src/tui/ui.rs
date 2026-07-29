use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::models::{Card, PlayedCard};

use super::app::{App, AppMode, ITEMS_PER_PAGE};
use super::render::{render_discarded_energy_line, render_hand_card, render_pokemon_card};

/// Slices `items` to the page indicated by `app.action_page`, returning `(page_items,
/// page_number, total_pages)` (1-based, for display). Clamps if the list shrank since the
/// current page was set.
fn paginate<T>(items: &[T], page: usize) -> (&[T], usize, usize) {
    let total_pages = items.len().div_ceil(ITEMS_PER_PAGE).max(1);
    let page = page.min(total_pages - 1);
    let start = (page * ITEMS_PER_PAGE).min(items.len());
    let end = (start + ITEMS_PER_PAGE).min(items.len());
    (&items[start..end], page + 1, total_pages)
}

/// Renders one board slot (active or bench). `title` is shown as the block's border title —
/// callers pass a seat-specific label (e.g. "P1 Active") so it's always correct regardless of
/// which seat is human or which side of the board it's drawn on.
fn render_board_slot(
    f: &mut Frame,
    area: Rect,
    pokemon: &Option<PlayedCard>,
    title: &str,
    player_color: Color,
) {
    let (lines, style, border_color, is_empty) = render_pokemon_card(pokemon, title, player_color);

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title_alignment(Alignment::Center)
        .title(title.to_string());
    if is_empty {
        block = block.border_type(BorderType::Rounded);
    }

    let pokemon_block = Paragraph::new(lines).style(style).block(block);
    f.render_widget(pokemon_block, area);
}

/// Renders up to 5 cards from `hand` (with left/right scroll arrows), starting at `scroll`.
/// `revealed` picks between showing real card names (a human's own hand) or "?" placeholders
/// (a concealed, non-human seat's hand).
fn render_hand_row(f: &mut Frame, chunks: &[Rect], hand: &[Card], scroll: usize, revealed: bool) {
    let total = hand.len();
    let start = scroll;
    let end = std::cmp::min(start + 5, total);
    let cards_to_show = end.saturating_sub(start);

    for i in 0..cards_to_show {
        let card_index = start + i;
        let left_arrow = if card_index == start && start > 0 {
            "←"
        } else {
            ""
        };
        let right_arrow = if card_index == end - 1 && end < total {
            "→"
        } else {
            ""
        };

        let (lines, style, title) = if revealed {
            let card = &hand[card_index];
            let (mut lines, style) = render_hand_card(card, card_index);
            if !left_arrow.is_empty() || !right_arrow.is_empty() {
                lines.insert(
                    0,
                    Line::from(vec![
                        Span::styled(
                            format!("{left_arrow} "),
                            Style::default().fg(Color::LightYellow),
                        ),
                        Span::styled(
                            format!(" {right_arrow}"),
                            Style::default().fg(Color::LightYellow),
                        ),
                    ]),
                );
            }
            (lines, style, "Hand".to_string())
        } else {
            let lines = vec![Line::from(vec![Span::styled(
                format!("{left_arrow} ? {right_arrow}"),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )])];
            (
                lines,
                Style::default().fg(Color::DarkGray),
                format!("#{}", card_index + 1),
            )
        };

        let hand_card_block = Paragraph::new(lines)
            .style(style)
            .alignment(if revealed {
                Alignment::Left
            } else {
                Alignment::Center
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title_alignment(Alignment::Center)
                    .title(title),
            );

        let chunk_index = 1 + (i * 2);
        f.render_widget(hand_card_block, chunks[chunk_index]);
    }
}

pub fn ui(f: &mut Frame, app: &App) {
    let state = app.get_state();
    let is_interactive = matches!(&app.mode, super::app::AppMode::Interactive { .. });

    // Main layout: left (battle log), center (game)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([
            Constraint::Percentage(25), // Battle log area
            Constraint::Percentage(75), // Game area
        ])
        .split(f.area());

    // Center: game area with battle mat, hand areas, and footer (no separate header)

    // `top_seat`/`bottom_seat` decide which seat's board renders where. `bottom_seat` is
    // whichever seat is "yours" (see `App::bottom_seat`): fixed at seat 1 unless you're
    // specifically playing seat 0 alone (`--players h,r`), in which case your own board
    // always renders at the bottom regardless of seat index. A seat's hand is drawn face-up
    // only if `App::is_hand_revealed` says so (human seats; in Replay mode, the bottom seat).
    let bottom_seat = app.bottom_seat();
    let top_seat = 1 - bottom_seat;

    // Adjust footer size based on mode - interactive mode needs more space for action list
    let footer_height = if is_interactive { 16 } else { 6 };
    // The top hand row is normally just a compact "?" placeholder (1 content line), but in
    // hot-seat play (both seats human) it can show real card names instead, which need the
    // same height as the bottom hand row.
    let top_hand_height = if app.is_hand_revealed(top_seat) { 5 } else { 3 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(top_hand_height), // Top hand
                Constraint::Min(0),                  // Battle mat
                Constraint::Length(5),               // Bottom hand
                Constraint::Length(footer_height), // Footer (larger in interactive mode for actions)
            ]
            .as_ref(),
        )
        .split(main_chunks[1]);

    let hand_row_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),     // Left padding
            Constraint::Length(18), // Card 1
            Constraint::Length(1),  // Spacing
            Constraint::Length(18), // Card 2
            Constraint::Length(1),  // Spacing
            Constraint::Length(18), // Card 3
            Constraint::Length(1),  // Spacing
            Constraint::Length(18), // Card 4
            Constraint::Length(1),  // Spacing
            Constraint::Length(18), // Card 5
            Constraint::Min(0),     // Right padding
        ]);

    render_hand_row(
        f,
        hand_row_chunks.clone().split(chunks[0]).as_ref(),
        &state.hands[top_seat],
        app.opponent_hand_scroll,
        app.is_hand_revealed(top_seat),
    );

    // Battle mat area - more compact for space efficiency
    let battle_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(8), // Top bench - compact but readable
                Constraint::Length(8), // Top active
                Constraint::Length(8), // Bottom active
                Constraint::Length(8), // Bottom bench
            ]
            .as_ref(),
        )
        .split(chunks[1]);

    let bench_chunks_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),     // Left padding
            Constraint::Length(24), // Bench 1
            Constraint::Length(1),  // Spacing
            Constraint::Length(24), // Bench 2
            Constraint::Length(1),  // Spacing
            Constraint::Length(24), // Bench 3
            Constraint::Min(0),     // Right padding
        ]);
    let active_area_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),     // Left padding (same as bench)
            Constraint::Length(24), // Bench 1 position (invisible)
            Constraint::Length(1),  // Spacing
            Constraint::Length(24), // Active (matches middle bench size)
            Constraint::Length(1),  // Spacing
            Constraint::Length(24), // Bench 3 position (invisible)
            Constraint::Min(0),     // Right padding (same as bench)
        ]);
    let bench_indices = [1, 3, 5]; // Skip spacing slots

    // Top bench
    let top_bench_chunks = bench_chunks_layout.clone().split(battle_area[0]);
    for (bench_pos, &chunk_idx) in bench_indices.iter().enumerate() {
        let pokemon = &state.in_play_pokemon[top_seat][bench_pos + 1];
        render_board_slot(
            f,
            top_bench_chunks[chunk_idx],
            pokemon,
            &format!("P{} Bench {}", top_seat + 1, bench_pos + 1),
            Color::Red,
        );
    }

    // Top active
    let top_active_area = active_area_layout.clone().split(battle_area[1]);
    let top_active = &state.in_play_pokemon[top_seat][0];
    render_board_slot(
        f,
        top_active_area[3],
        top_active,
        &format!("P{} Active", top_seat + 1),
        Color::Red,
    );

    // Bottom active
    let bottom_active_area = active_area_layout.split(battle_area[2]);
    let bottom_active = &state.in_play_pokemon[bottom_seat][0];
    render_board_slot(
        f,
        bottom_active_area[3],
        bottom_active,
        &format!("P{} Active", bottom_seat + 1),
        Color::Green,
    );

    // Bottom bench
    let bottom_bench_chunks = bench_chunks_layout.split(battle_area[3]);
    for (bench_pos, &chunk_idx) in bench_indices.iter().enumerate() {
        let pokemon = &state.in_play_pokemon[bottom_seat][bench_pos + 1];
        render_board_slot(
            f,
            bottom_bench_chunks[chunk_idx],
            pokemon,
            &format!("P{} Bench {}", bottom_seat + 1, bench_pos + 1),
            Color::Green,
        );
    }

    render_hand_row(
        f,
        hand_row_chunks.split(chunks[2]).as_ref(),
        &state.hands[bottom_seat],
        app.player_hand_scroll,
        app.is_hand_revealed(bottom_seat),
    );

    // Footer with game status and possible actions
    let actor = app.get_current_actor();
    let actions = app.get_possible_actions();

    // Build discarded energy display
    let p1_discard_line = render_discarded_energy_line(&state.discard_energies[0]);
    let p2_discard_line = render_discarded_energy_line(&state.discard_energies[1]);

    // Build header line with game status
    let header_line = if is_interactive {
        format!(
            "DeckGym [INTERACTIVE] | Turn: {} | P1: {} pts | P2: {} pts",
            state.turn_count, state.points[0], state.points[1]
        )
    } else {
        format!(
            "DeckGym [REPLAY] State: {}/{} | Turn: {} | P1: {} pts | P2: {} pts",
            app.get_current_state_index() + 1,
            app.get_states_len(),
            state.turn_count,
            state.points[0],
            state.points[1]
        )
    };

    let footer_lines = if is_interactive {
        // Interactive mode footer
        let current_actor = app.get_current_actor();
        let is_human_turn = app.is_current_actor_human();

        let mut lines = vec![
            Line::from(vec![Span::styled(
                "P1 Discard: ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )])
            .spans
            .into_iter()
            .chain(p1_discard_line.spans.into_iter())
            .collect::<Vec<_>>()
            .into(),
            Line::from(vec![Span::styled(
                "P2 Discard: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )])
            .spans
            .into_iter()
            .chain(p2_discard_line.spans.into_iter())
            .collect::<Vec<_>>()
            .into(),
        ];

        if is_human_turn {
            let paging_hint =
                if app.get_draw_choice().map_or(actions.len(), |c| c.len()) > ITEMS_PER_PAGE {
                    ", PageUp/PageDown=more"
                } else {
                    ""
                };
            lines.push(Line::from(format!("Controls: ESC/q=quit, 1-9=select{paging_hint}, u/Backspace=undo, W/S=jump turn, Left/Right=scroll player hand, A/D=scroll opp hand")));

            if let Some(candidates) = app.get_draw_choice() {
                let (page_candidates, page_num, total_pages) =
                    paginate(candidates, app.action_page);
                let title = if total_pages > 1 {
                    format!(
                        "P{} TURN - Choose a card to draw (page {}/{}):",
                        current_actor + 1,
                        page_num,
                        total_pages
                    )
                } else {
                    format!("P{} TURN - Choose a card to draw:", current_actor + 1)
                };
                lines.push(Line::from(vec![Span::styled(
                    title,
                    Style::default()
                        .fg(Color::LightYellow)
                        .add_modifier(Modifier::BOLD),
                )]));

                if candidates.is_empty() {
                    lines.push(Line::from("No cards left in deck"));
                } else {
                    for (i, card) in page_candidates.iter().enumerate() {
                        lines.push(Line::from(vec![Span::styled(
                            format!("{}. {}", i + 1, card.get_name()),
                            Style::default().fg(Color::White),
                        )]));
                    }
                }
            } else {
                let (page_actions, page_num, total_pages) = paginate(&actions, app.action_page);
                let title = if total_pages > 1 {
                    format!(
                        "P{} TURN - Select Action (page {}/{}):",
                        current_actor + 1,
                        page_num,
                        total_pages
                    )
                } else {
                    format!("P{} TURN - Select Action:", current_actor + 1)
                };
                lines.push(Line::from(vec![Span::styled(
                    title,
                    Style::default()
                        .fg(Color::LightYellow)
                        .add_modifier(Modifier::BOLD),
                )]));

                if actions.is_empty() {
                    lines.push(Line::from("No actions available"));
                } else {
                    for (i, action) in page_actions.iter().enumerate() {
                        lines.push(Line::from(vec![Span::styled(
                            format!("{}. {:?}", i + 1, action.action),
                            Style::default().fg(Color::White),
                        )]));
                    }
                }
            }
        } else {
            // Non-human seat's turn - show waiting message
            lines.push(Line::from("Controls: ESC/q=quit, u/Backspace=undo, W/S=jump turn, Left/Right=scroll player hand, A/D=scroll opp hand"));
            lines.push(Line::from(vec![Span::styled(
                format!("P{} (BOT) TURN - Waiting...", current_actor + 1),
                Style::default().fg(Color::Yellow),
            )]));
        }
        lines
    } else {
        // Replay mode footer
        let action_strings: Vec<String> = actions
            .iter()
            .take(10)
            .map(|a| format!("{:?}", a.action))
            .collect();

        let actions_text = if action_strings.is_empty() {
            "No actions available".to_string()
        } else {
            action_strings.join(" | ")
        };

        vec![
            Line::from(vec![
                Span::styled("P1 Discard: ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            ]).spans.into_iter().chain(p1_discard_line.spans.into_iter()).collect::<Vec<_>>().into(),
            Line::from(vec![
                Span::styled("P2 Discard: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]).spans.into_iter().chain(p2_discard_line.spans.into_iter()).collect::<Vec<_>>().into(),
            Line::from("Controls: ESC/q=quit, Up/Down=navigate states, W/S=jump turn, Left/Right=scroll player hand, A/D=scroll opp hand, C=toggle center"),
            Line::from(format!("Current Player: P{}", actor + 1)),
            Line::from(format!("Possible Actions: {}", actions_text)),
        ]
    };

    let footer = Paragraph::new(footer_lines)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL).title(header_line));
    f.render_widget(footer, chunks[3]);

    // Left side: Battle log panel with actions
    let mut log_lines = Vec::new();
    let mut turn_log_lines = Vec::new(); // Track line numbers where turn headers appear
    let actions = app.get_actions();

    // Track where the "CURRENT" marker is placed in the log_lines vector
    let mut current_marker_line: Option<usize> = None;

    if is_interactive {
        // Interactive mode - live battle log
        let turn_history = app.get_turn_history().unwrap_or_default();
        let mut current_turn: u8 = 0;

        // Initial header
        turn_log_lines.push(log_lines.len());
        log_lines.push(Line::from(vec![Span::styled(
            "━━━ Setup Phase ━━━",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]));

        for (i, action) in actions.iter().enumerate() {
            let player_num = action.actor;
            let player_color = if player_num == 0 {
                Color::Red
            } else {
                Color::Green
            };

            // Check if turn changed before this action
            if i < turn_history.len() {
                let action_turn = turn_history[i];
                if action_turn != current_turn {
                    current_turn = action_turn;
                    log_lines.push(Line::from(""));
                    turn_log_lines.push(log_lines.len());
                    let header = if current_turn == 0 {
                        "━━━ Setup Phase ━━━".to_string()
                    } else {
                        format!("━━━ Turn {} ━━━", current_turn)
                    };
                    log_lines.push(Line::from(vec![Span::styled(
                        header,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )]));
                }
            }

            // Add the action line
            log_lines.push(Line::from(vec![
                Span::styled(
                    format!("P{}: ", player_num + 1),
                    Style::default()
                        .fg(player_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}", action.action),
                    Style::default().fg(Color::White),
                ),
            ]));
        }

        // Show current turn header at the end if it's different
        if state.turn_count != current_turn {
            log_lines.push(Line::from(""));
            turn_log_lines.push(log_lines.len());
            let header = if state.turn_count == 0 {
                "━━━ Setup Phase ━━━".to_string()
            } else {
                format!("━━━ Turn {} ━━━", state.turn_count)
            };
            log_lines.push(Line::from(vec![Span::styled(
                header,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]));
        }
    } else {
        // Replay mode - show full history with turn headers
        if let AppMode::Replay {
            states,
            current_index,
            ..
        } = &app.mode
        {
            let mut current_turn: u8 = if !states.is_empty() {
                states[0].turn_count
            } else {
                0
            };

            // Add initial turn header
            turn_log_lines.push(log_lines.len());
            let header = if current_turn == 0 {
                "━━━ Setup Phase ━━━".to_string()
            } else {
                format!("━━━ Turn {current_turn} ━━━")
            };
            log_lines.push(Line::from(vec![Span::styled(
                header,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]));

            for (i, action) in actions.iter().enumerate() {
                let player_num = action.actor;
                let player_color = if player_num == 0 {
                    Color::Red
                } else {
                    Color::Green
                };

                // Add cursor indicator before this action if we're between state i and i+1
                if i == *current_index && i < actions.len() {
                    current_marker_line = Some(log_lines.len());
                    log_lines.push(Line::from(vec![Span::styled(
                        ">>> CURRENT <<<",
                        Style::default()
                            .fg(Color::LightYellow)
                            .add_modifier(Modifier::BOLD),
                    )]));
                }

                // Add the action line
                log_lines.push(Line::from(vec![
                    Span::styled(
                        format!("P{}: ", player_num + 1),
                        Style::default()
                            .fg(player_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{}", action.action),
                        Style::default().fg(Color::White),
                    ),
                ]));

                // Check if turn changed after this action
                if i + 1 < states.len() {
                    let next_turn = states[i + 1].turn_count;

                    if next_turn != current_turn && i + 1 < actions.len() {
                        log_lines.push(Line::from(""));
                        turn_log_lines.push(log_lines.len());
                        let header = if next_turn == 0 {
                            "━━━ Setup Phase ━━━".to_string()
                        } else {
                            format!("━━━ Turn {next_turn} ━━━")
                        };
                        log_lines.push(Line::from(vec![Span::styled(
                            header,
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        )]));
                    }

                    current_turn = next_turn;
                }
            }

            // If we're at the initial state and there are no actions yet, or at the final state
            if current_marker_line.is_none() && *current_index == actions.len() {
                current_marker_line = Some(log_lines.len());
                log_lines.push(Line::from(vec![Span::styled(
                    ">>> CURRENT <<<",
                    Style::default()
                        .fg(Color::LightYellow)
                        .add_modifier(Modifier::BOLD),
                )]));

                if actions.is_empty() {
                    log_lines.push(Line::from("Game Start"));
                }
            }
        }
    }

    // If the game has ended, add "Game over" header to the log
    if state.is_game_over() {
        log_lines.push(Line::from(""));
        log_lines.push(Line::from(vec![Span::styled(
            "━━━ Game over ━━━",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )]));
    }

    // Adjust scroll to center around current marker in battle log if flag is on
    let mut battle_log_scroll = app.scroll_offset;
    if app.lock_actions_center {
        if let Some(marker_idx) = current_marker_line {
            // Visible lines inside the block - account for borders (2 lines)
            let area_height = main_chunks[0].height as usize;
            let visible = area_height.saturating_sub(2).max(1);
            let total_lines = log_lines.len();

            // Desired top line so marker is centered
            let desired_top = marker_idx.saturating_sub(visible / 2);
            let max_top = total_lines.saturating_sub(visible);
            let top = std::cmp::min(desired_top, max_top);
            battle_log_scroll = top as u16;
        }
    }

    let battle_log = Paragraph::new(log_lines)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL).title("Battle Log"))
        .scroll((battle_log_scroll, 0));
    f.render_widget(battle_log, main_chunks[0]);
}
