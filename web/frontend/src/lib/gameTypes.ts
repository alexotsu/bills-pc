// TypeScript mirror of the engine's State/Card/Action shapes as they cross the wasm boundary
// (serde's default JSON encoding — see web/engine-wasm/src/lib.rs). Fields that are purely
// internal engine bookkeeping (never rendered) are typed loosely as `unknown` rather than
// exhaustively — see individual comments below for why each one is safe to leave loose.

export type EnergyType =
  | "Grass"
  | "Fire"
  | "Water"
  | "Lightning"
  | "Psychic"
  | "Fighting"
  | "Darkness"
  | "Metal"
  | "Dragon"
  | "Colorless";

export type Attack = {
  energy_required: EnergyType[];
  title: string;
  fixed_damage: number;
  effect: string | null;
};

export type Ability = { title: string; effect: string };

export type PokemonCard = {
  id: string;
  name: string;
  stage: number;
  evolves_from: string | null;
  hp: number;
  energy_type: EnergyType;
  ability: Ability | null;
  attacks: Attack[];
  weakness: EnergyType | null;
  retreat_cost: EnergyType[];
  rarity: string;
  booster_pack: string;
};

export type TrainerType = "Supporter" | "Item" | "Tool" | "Fossil" | "Stadium";

export type TrainerCard = {
  id: string;
  trainer_card_type: TrainerType;
  name: string;
  effect: string;
  rarity: string;
  booster_pack: string;
};

/** `Card::Pokemon`/`Card::Trainer` — serde's default externally-tagged encoding. */
export type Card = { Pokemon: PokemonCard } | { Trainer: TrainerCard };

export function isPokemonCard(card: Card): card is { Pokemon: PokemonCard } {
  return "Pokemon" in card;
}

export function cardName(card: Card): string {
  return isPokemonCard(card) ? card.Pokemon.name : card.Trainer.name;
}

/** `card.rs`'s `is_ex()`/`is_mega()` are computed, not stored fields — mirrored here exactly. */
export function isExCard(name: string): boolean {
  const lastWord = name.toLowerCase().trim().split(" ").pop();
  return lastWord === "ex";
}

/** A Pokémon currently in play (active or bench). `damage_counters`/`base_hp`/`stadium_hp_bonus`
 * and the 5 status booleans are private fields in Rust but still serialize — see
 * `src/state/played_card.rs`. `effective_total_hp` is computed engine-side (`base_hp` plus any
 * HP-boosting tool/ability bonus, e.g. Giant Cape, Leaf Cape, Reuniclus's Infinite Increase) via
 * `get_effective_total_hp()` — use it (through `effectiveTotalHp` below) rather than deriving
 * total HP from `base_hp`/`stadium_hp_bonus` here, which would silently miss those bonuses.
 * Remaining HP isn't a stored field; compute it like the engine does (`get_remaining_hp` =
 * effective total HP minus damage) via `remainingHp` below. */
export type PlayedCard = {
  card: Card;
  damage_counters: number;
  base_hp: number;
  stadium_hp_bonus: number;
  effective_total_hp: number;
  attached_energy: EnergyType[];
  attached_tool: Card | null;
  played_this_turn: boolean;
  moved_to_active_this_turn: boolean;
  ability_used: boolean;
  poisoned: boolean;
  paralyzed: boolean;
  asleep: boolean;
  burned: boolean;
  confused: boolean;
  cards_behind: Card[];
  prevent_first_attack_damage_used: boolean;
  has_attacked_since_play: boolean;
  // Internal effect-tracking (CardEffect, duration) — not surfaced in the UI.
  effects: unknown[];
};

export function effectiveTotalHp(played: PlayedCard): number {
  return played.effective_total_hp;
}

export function remainingHp(played: PlayedCard): number {
  return effectiveTotalHp(played) - played.damage_counters;
}

export type EnergyZoneState = { current: EnergyType | null; next: EnergyType | null };

export type DeckState = { cards: Card[]; energy_types: EnergyType[] };

export type GameOutcome = { Win: number } | "Tie";

export type DrawSource = "InitialHand" | "TurnStart" | "Ability" | "Attack";

/** Full engine `State` (`src/state/mod.rs`). Fields never rendered by the board
 * (`move_generation_stack`, `attack_name_used_count`, `turn_effects`) are typed loosely —
 * they're transient/internal resolution bookkeeping, not board-visible information. */
export type State = {
  winner: GameOutcome | null;
  points: [number, number];
  turn_count: number;
  current_player: number;
  end_turn_pending: boolean;
  move_generation_stack: unknown[];
  energy_zone: [EnergyZoneState, EnergyZoneState];
  hands: [Card[], Card[]];
  decks: [DeckState, DeckState];
  discard_piles: [Card[], Card[]];
  discard_energies: [EnergyType[], EnergyType[]];
  in_play_pokemon: [
    [PlayedCard | null, PlayedCard | null, PlayedCard | null, PlayedCard | null],
    [PlayedCard | null, PlayedCard | null, PlayedCard | null, PlayedCard | null],
  ];
  active_stadium: Card | null;
  active_stadium_owner: number | null;
  has_played_support: boolean;
  has_retreated: boolean;
  has_used_stadium: [boolean, boolean];
  knocked_out_by_opponent_attack_this_turn: boolean;
  knocked_out_by_opponent_attack_last_turn: boolean;
  attack_name_used_this_turn: [string | null, string | null];
  attack_name_used_last_turn: [string | null, string | null];
  attack_name_used_count: unknown;
  turn_effects: unknown;
};

/** `SimpleAction` (`src/actions/types.rs`) has 30+ variants, most of them one-off card-specific
 * mechanics. Rather than exhaustively typing every one (a maintenance burden that grows with
 * every new card implemented), this types the shape generically — unit variants serialize as a
 * bare string, data-carrying variants as `{ VariantName: payload }` — and `describeAction.ts`
 * handles the common cases by name with a generic fallback for the rest, so a legal action is
 * never hidden just because its specific variant isn't hand-cased. */
export type SimpleAction = string | Record<string, unknown>;

export type Action = {
  actor: number;
  action: SimpleAction;
  is_stack: boolean;
};

export type PendingDecision =
  | { kind: "awaiting_action"; actor: number; actions: Action[] }
  | { kind: "awaiting_draw"; actor: number; source: DrawSource; amount: number }
  // `GameOver` is a struct variant (`{ outcome: Option<GameOutcome> }`) in Rust, not a
  // tuple/newtype one — see the comment on it in src/game/interactive.rs. serde's internally
  // tagged (`tag = "kind"`) representation can't serialize a newtype variant whose payload
  // isn't itself an object, which `Option<GameOutcome>` isn't; the struct-variant form avoids
  // that entirely, at the cost of needing a named field here rather than a positional one.
  | { kind: "game_over"; outcome: GameOutcome | null };
