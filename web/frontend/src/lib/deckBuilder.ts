import type { CardCatalogEntry } from "@/lib/api";

const DECK_SIZE = 20;
const MAX_COPIES_PER_NAME = 2;

/** Card id ("SET NUMBER", e.g. "A1 001") -> how many copies of that specific printing are in
 * the deck. Kept per-id, not per-name: the same-named Pokémon can be reprinted across sets with
 * a *different moveset* each time, so the picker needs to let you choose a specific printing,
 * not just "the name." What's per-name is only the deck-building *rule* — at most 2 total copies
 * across every id that shares a name, matching `Deck::is_valid()` (`src/deck.rs`), which keys
 * its count by `card.get_name()`, not by id. See `countsByName` below. */
export type CardCounts = Record<string, number>;

/** Strips diacritics before comparing, so searching "poke" finds "Poké Ball". */
export function normalizeForSearch(value: string): string {
  return value
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();
}

/** Sums per-id counts into per-name totals — the unit the "at most 2" rule actually applies to,
 * and what the picker needs to know to stop you adding a 3rd copy of a name via a *different*
 * printing once you already have 2 of another. */
export function countsByName(
  counts: CardCounts,
  catalog: CardCatalogEntry[],
): Record<string, number> {
  const byId = new Map(catalog.map((c) => [c.id, c]));
  const totals: Record<string, number> = {};
  for (const [id, count] of Object.entries(counts)) {
    const name = byId.get(id)?.name;
    if (!name) continue;
    totals[name] = (totals[name] ?? 0) + count;
  }
  return totals;
}

/**
 * Renders counts back into the DeckGym text format `Deck::from_string` parses. The "Pokémon: N"
 * / "Trainer: M" header lines are cosmetic only — the parser skips them — but are included for
 * readability and parity with what deckgym.com's builder exports.
 */
export function buildDeckText(counts: CardCounts, catalog: CardCatalogEntry[]): string {
  const byId = new Map(catalog.map((c) => [c.id, c]));
  const selected = Object.entries(counts)
    .filter(([, count]) => count > 0)
    .sort(([a], [b]) => a.localeCompare(b));

  const line = ([id, count]: [string, number]) => {
    const card = byId.get(id);
    return card ? `${count} ${card.name} ${card.id}` : null;
  };

  const pokemon = selected.filter(([id]) => byId.get(id)?.card_type === "pokemon");
  const trainers = selected.filter(([id]) => byId.get(id)?.card_type === "trainer");
  const pokemonCount = pokemon.reduce((sum, [, c]) => sum + c, 0);
  const trainerCount = trainers.reduce((sum, [, c]) => sum + c, 0);

  const sections: string[] = [];
  if (pokemon.length > 0) {
    sections.push([`Pokémon: ${pokemonCount}`, ...pokemon.map(line)].join("\n"));
  }
  if (trainers.length > 0) {
    sections.push([`Trainer: ${trainerCount}`, ...trainers.map(line)].join("\n"));
  }
  return sections.join("\n\n");
}

/**
 * The inverse of `buildDeckText` — reconstructs per-id counts from stored `deck_text`, so the
 * edit page can preload a deck's existing contents into the picker. Mirrors `Deck::from_string`'s
 * line parsing (`src/deck.rs`): skip blank lines and "Pokémon:"/"Trainer:"/"Energy:" headers,
 * otherwise take the last two whitespace-separated tokens as the card id.
 */
export function parseDeckText(deckText: string): CardCounts {
  const counts: CardCounts = {};
  for (const rawLine of deckText.split("\n")) {
    const trimmed = rawLine.trim();
    if (
      trimmed === "" ||
      trimmed.startsWith("Pokémon:") ||
      trimmed.startsWith("Trainer:") ||
      trimmed.startsWith("Energy:")
    ) {
      continue;
    }
    const parts = trimmed.split(/\s+/);
    if (parts.length < 3) continue;
    const count = parseInt(parts[0], 10);
    if (Number.isNaN(count)) continue;
    const set = parts[parts.length - 2];
    const number = parts[parts.length - 1].padStart(3, "0");
    const id = `${set} ${number}`;
    counts[id] = (counts[id] ?? 0) + count;
  }
  return counts;
}

export type DeckValidity = {
  totalCards: number;
  basicCount: number;
  isValid: boolean;
  errors: string[];
};

/** Client-side mirror of `Deck::is_valid()` (`src/deck.rs`), for instant feedback before the
 * authoritative server-side check on save. */
export function checkDeckValidity(counts: CardCounts, catalog: CardCatalogEntry[]): DeckValidity {
  const byId = new Map(catalog.map((c) => [c.id, c]));
  const totalCards = Object.values(counts).reduce((sum, n) => sum + n, 0);
  const basicCount = Object.entries(counts).reduce((sum, [id, n]) => {
    return byId.get(id)?.is_basic ? sum + n : sum;
  }, 0);

  const errors: string[] = [];
  if (totalCards !== DECK_SIZE) {
    errors.push(`Deck has ${totalCards} of ${DECK_SIZE} cards.`);
  }
  if (basicCount < 1) {
    errors.push("Deck needs at least 1 Basic Pokémon.");
  }
  for (const [name, total] of Object.entries(countsByName(counts, catalog))) {
    if (total > MAX_COPIES_PER_NAME) {
      errors.push(
        `${name}: at most ${MAX_COPIES_PER_NAME} copies allowed across all printings ` +
          `(currently ${total}).`,
      );
    }
  }

  return { totalCards, basicCount, isValid: errors.length === 0, errors };
}

export { DECK_SIZE, MAX_COPIES_PER_NAME };
