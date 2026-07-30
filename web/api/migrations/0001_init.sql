-- First-draft schema (web/SPEC.md). Expected to evolve once real auth/deck-CRUD work starts.

create extension if not exists "pgcrypto";

create table users (
    id uuid primary key default gen_random_uuid(),
    email text unique,
    password_hash text,
    oauth_provider text,
    oauth_subject text,
    -- Required at account-creation time (see SPEC.md: no opt-in, no account). Deleting an
    -- account removes PII but never this flag's downstream data — training data is retained
    -- regardless of account deletion, since consent was already given at signup.
    training_data_opt_in boolean not null,
    created_at timestamptz not null default now()
);

-- Deck contents are stored as the existing DeckGym text format (same format `Deck::from_string`
-- parses, and the same one deckgym.com's builder exports via "Copy as Text") rather than a new
-- schema, so decks can be validated/loaded with the engine's existing parser as-is.
create table decks (
    id uuid primary key default gen_random_uuid(),
    user_id uuid references users(id) on delete cascade,
    name text not null,
    deck_text text not null,
    -- Reference decks (SPEC.md's curated meta-deck list) are admin-seeded rows with no owner.
    is_reference boolean not null default false,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table games (
    id uuid primary key default gen_random_uuid(),
    user_id uuid references users(id) on delete cascade,
    deck_a_id uuid not null references decks(id),
    deck_b_id uuid not null references decks(id),
    mode text not null check (mode in ('hotseat', 'ai')),
    outcome text check (outcome in ('win', 'loss', 'tie', 'incomplete')),
    seed bigint not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- Mirrors the engine's existing ExportedDataPoint schema (src/data_exporter.rs) field-for-field
-- — the same (state, playable_actions, chosen_action) trace already proven for CLI data export.
create table game_plies (
    id bigserial primary key,
    game_id uuid not null references games(id) on delete cascade,
    ply integer not null,
    actor smallint not null,
    state_json jsonb not null,
    playable_actions_json jsonb not null,
    chosen_action_json jsonb not null,
    created_at timestamptz not null default now(),
    unique (game_id, ply)
);

create index game_plies_game_id_idx on game_plies (game_id);
create index games_user_id_idx on games (user_id);
create index decks_user_id_idx on decks (user_id);
