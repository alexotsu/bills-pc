# Web Interface — Feature Spec

## Goal

A web interface for the deckgym-core engine that lets anyone create and save decks, then
play games against themselves (or an AI) in a faster, more controlled environment than
playing the actual game allows. The eventual goal is to collect enough real-player gameplay
data to train a model that plays like an actual person — every design decision below is in
service of that: fast iteration for the player, and clean, complete data capture for training.

## Board layout

Matches the real game's layout and extends what the TUI already renders — top-to-bottom:
opponent hand → opponent bench (3) → opponent active → stadium (shared, center) → player
active → player bench (3) → player hand, with an energy zone and discard pile visible per
side.

Per-Pokémon, show: name, HP bar, attached energy (icons), attached tool (if any), move
name(s) + damage + energy cost, weakness, retreat cost, status conditions (poisoned/
asleep/paralyzed/burned/confused).

Two pieces of this go beyond what the TUI currently displays but need **no new engine
work** — the data already exists in `State`, this is purely a frontend rendering task:

- **Energy zone "next" preview** — `energy_zone[player].next` already exists (same field
  the Python bindings expose as `next_energy`).
- **Discard pile as a real pile of cards**, not just an energy-count summary —
  `discard_piles`/`discard_energies` already track this; the TUI only ever surfaced the
  energy-count line, not the actual discarded cards.
- **Stadium slot** — `active_stadium`/`active_stadium_owner` already exist in `State`, just
  never got a TUI panel.

## Accounts

- Email+password or OAuth (Google, Facebook) login.
- Account creation requires checking a box opting into training-data collection — no
  opt-in, no account. This makes consent unambiguous and avoids the GDPR
  purpose-limitation problem entirely, since there's no "opted out but has data" state to
  reconcile.
- Deletion removes PII; gameplay/training data is retained regardless (already covered by
  the opt-in — there's no separate "please keep my data anyway" case to design for).

## Decks

- Create/name/edit decks, validated against the engine's existing deck rules (20 cards,
  ≤2 copies per named card, ≥1 Basic Pokémon) with inline errors, not silent rejection.
- Immutable (or versioned) once a deck has recorded battle history, so stats stay
  attributable to the deck list that actually produced them.
- Deck builder excludes cards the engine doesn't fully implement yet (reuse its existing
  implementation-status check as the single source of truth), shown grayed-out with a
  reason rather than hidden entirely.
- Card images keyed per card ID (not name — alt-art/full-art variants need distinct
  images), placeholder for now and easy to swap once a source is found. Image
  sourcing/licensing is a pre-launch legal question (official card art is copyrighted),
  not just a technical swap-out.

## Reference decks

- A curated, admin-editable (config file for v1) list of ~10 meta decks, usable as the
  opponent deck in *either* AI mode or hotseat mode. Purely a convenience feature so users
  aren't rebuilding the same decks from scratch.

## Playing a game

- Per-game settings: **opponent mode** (hotseat, default / AI, difficulty = engine's
  existing bot players) and **draw override** (choose your next card instead of drawing
  randomly).
- **Hotseat**: both sides human. Only the current player's hand is revealed; the other is
  hidden. Advance turns via an explicit **"End Player \<N\>'s Turn"** button — no
  interstitial/handoff screen needed. There is no anti-cheat concern, since results don't
  gate anything (no prizes/ranking) — hiding exists purely so each side's decisions are
  made under the same uncertainty a real game would have, for data-quality reasons, not
  security.
- **AI mode**: the opponent's hand is never sent to the client at all — genuine hidden
  information, not just a UI convention.
- Executes **client-side** (compiled to WASM) for zero-latency actions, matching the "fast
  feedback loop" goal directly. Periodic background sync pushes state to the backend for
  persistence.
- Undo is available (the engine already supports this via state snapshots).
- Manual "mark as won / lost / tied" ends a game early with a confirmation step (it's
  destructive to the in-progress game).
- Fast board animations: a border flash (default ~100ms, configurable) around every
  affected Pokémon for any board-changing action — Pokémon played, evolved, attacked,
  energy attached, tool attached, healed, discarded, knocked out, status applied. Effects
  affecting multiple Pokémon at once animate as one synchronized batch, not sequential
  per-target flashes.

## Battle history

- Every game persists the full per-ply `(state, legal actions, chosen action)` trace —
  reusing the engine's existing export schema — not just the final win/loss outcome,
  synced incrementally so incomplete/abandoned games aren't lost.
- Each ply is tagged with which mode produced it (hotseat vs. AI) so the training pipeline
  can filter or weight accordingly.
- History view: filterable by outcome (won/lost/incomplete) and sliceable by deck. Each
  entry links to a full replay (reusing the TUI's Replay-mode concept).
- Per-deck win/loss/tie record against each opponent deck faced. Scope (per-user vs.
  global aggregate across all users) is still open — low-stakes to decide later, since
  it's a query/aggregation choice, not a schema one.

## Open questions

- Per-deck battle history: scoped per-user, or aggregated globally across all users who've
  played that exact deck configuration?
- Exact retention window for PII deletion requests (GDPR Art. 17 doesn't mandate a
  specific number of days, but a concrete SLA should be documented).
- Card image source/license once ready to move off placeholders.
