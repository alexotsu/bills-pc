# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **Interactive mode control plane** (`Game::step`/`submit_action`/`submit_draw`): lets an external caller drive either or both seats turn-by-turn instead of always going through a `Player`'s `decision_fn`. Opt in per seat with `Game::set_interactive`; every existing caller of `Game::new` is unaffected by default.
  - `PendingDecision` (`AwaitingAction`, `AwaitingDraw`, `GameOver`) reports what the game is waiting on next.
  - `InteractiveConfig { override_draws }` additionally lets a seat's opening-hand and turn-start draws pause for an explicit card choice (`submit_draw`) instead of resolving randomly.
  - New `DrawSource` tag (`InitialHand`, `TurnStart`, `Ability`, `Attack`) on `SimpleAction::DrawCard` records why a draw was queued, so the override machinery — and future extensions to it — can target specific draw sources. This is a visible, additive change to the exported JSON action format.
  - `InteractivePlayer`: a placeholder `Player` for interactive seats that panics if its `decision_fn` is ever actually invoked, as a tripwire against interception bugs.
  - Interactively-driven games export identically to bulk-simulated ones (same `SimulationEventHandler`/`DataExporter` hook).
  - Python bindings: `PyGame.set_interactive`/`set_scripted`/`step`/`submit_action`/`submit_draw`, plus a `"interactive"` sentinel recognized in `PyGame`'s `players` list.
- **TUI: play interactively as either or both players.** `--players` now honors `h` in any seat, including `h,h` for local hot-seat play (previously only `--players <bot>,h` worked; other combinations could hang the terminal).
  - New `--override-draws` flag: lets a human seat choose which card to draw (opening hand and every turn) instead of drawing automatically. Defaults to off (normal random draw).
  - New undo command (`u` or Backspace): rolls the game back one applied action at a time — yours or the bot's — for recovering from a misclick. Press repeatedly to go back further.
  - Action and draw-choice lists now page through PageUp/PageDown when there are more than 9 options, instead of silently truncating with no way to reach the rest.

### Fixed

- `generate_possible_actions` checked the setup phase before the forced-decision stack, so a follow-up choice triggered during setup (e.g. Miraidon ex's Legendary Drive when benched before turn 1) was silently dropped until turn 1. The stack is now checked first.
- TUI board/hand rendering was hardcoded to treat seat 0 as "the opponent" and seat 1 as "you." Playing seat 0 (`--players h,r`) showed your own hand as hidden "?" cards while revealing the bot's hand. Your own board and hand now always render in the same place regardless of which seat you're actually playing, and panels are labeled "P1"/"P2" instead of an unlabeled, color-only distinction.

### Changed

- `State::initialize`'s 10 opening-hand draws are now queued (`DrawSource::InitialHand`) and resolved through the normal tick loop instead of a synchronous loop inside `Game::new` — required so they can be intercepted for draw override. Verified not to change the RNG stream for any existing seeded test or simulation.
