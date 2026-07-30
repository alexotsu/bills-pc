import { cardName, isPokemonCard, type Action, type Card, type State } from "@/lib/gameTypes";

/** "DiscardOwnBenchedThenDamage" -> "Discard Own Benched Then Damage" */
function humanizeVariantName(name: string): string {
  return name.replace(/([a-z0-9])([A-Z])/g, "$1 $2");
}

function isCard(value: unknown): value is Card {
  return (
    typeof value === "object" && value !== null && ("Pokemon" in value || "Trainer" in value)
  );
}

function slotLabel(idx: number): string {
  return idx === 0 ? "Active" : `Bench ${idx}`;
}

/** "Active Bulbasaur" / "Bench 2 (empty)" — the slot name plus whichever Pokémon currently
 * occupies it, so choosing between "Attach Energy" options for 3 different Pokémon (say) is
 * actually distinguishable instead of 3 identical-looking buttons. */
function pokemonAtSlot(state: State, boardOwner: number, idx: number): string {
  const slot = slotLabel(idx);
  const played = state.in_play_pokemon[boardOwner]?.[idx];
  if (!played) return `${slot} (empty)`;
  const name = isPokemonCard(played.card) ? played.card.Pokemon.name : cardName(played.card);
  return `${slot} ${name}`;
}

/** Best-effort context for the generic fallback: pulls a card name out of whichever
 * commonly-used field the payload happens to have (varies by action — see SimpleAction in
 * src/actions/types.rs), so even actions with no hand-written case below stay readable rather
 * than just showing a bare variant name. */
function extractCardContext(payload: Record<string, unknown>): string | null {
  const singleCardFields = ["card", "hand_pokemon", "supporter_card", "tool_card", "evolution"];
  for (const field of singleCardFields) {
    const value = payload[field];
    if (isCard(value)) return cardName(value);
  }
  const cardListFields = ["cards", "hand_pokemon"];
  for (const field of cardListFields) {
    const value = payload[field];
    if (Array.isArray(value) && value.every(isCard)) {
      return value.map(cardName).join(", ");
    }
  }
  return null;
}

/**
 * Turns an `Action` into a human-readable button label. Hand-cases the common, always-present
 * actions; everything else (the many one-off card-specific mechanics in `src/actions/types.rs`)
 * falls back to a humanized variant name plus any card name found in its payload — so a legal
 * action is never hidden from the action list just because its specific variant isn't
 * hand-cased here, which matters since new cards add new variants regularly.
 *
 * `state` is needed (not just the action itself) so actions that target a specific in-play slot
 * — most of them carry an `in_play_idx` rather than a card, since the card doesn't move — can
 * say *which* Pokémon they mean instead of just "Attach Energy" three times over with no way to
 * tell the buttons apart.
 */
export function describeAction(action: Action, state: State): string {
  const simpleAction = action.action;
  if (typeof simpleAction === "string") {
    if (simpleAction === "EndTurn") return "End Turn";
    if (simpleAction === "UseStadium") return "Use Stadium";
    if (simpleAction === "Noop") return "Pass";
    if (simpleAction === "DiscardActiveStadium") return "Discard Active Stadium";
    if (simpleAction === "DiscardRandomOpponentActiveEnergy") {
      return "Discard a Random Energy from Opponent's Active";
    }
    if (simpleAction === "ApplyEeveeBagDamageBoost") return "Apply Eevee Bag Damage Boost";
    if (simpleAction === "HealAllEeveeEvolutions") return "Heal All Eevee Evolutions";
    return humanizeVariantName(simpleAction);
  }

  const entries = Object.entries(simpleAction);
  if (entries.length !== 1) return "Unknown action";
  const [variant, rawPayload] = entries[0];
  const payload = (rawPayload ?? {}) as Record<string, unknown>;
  const at = (idx: number, owner: number = action.actor) => pokemonAtSlot(state, owner, idx);

  switch (variant) {
    case "DrawCard":
      return `Draw ${payload.amount ?? 1} Card${(payload.amount as number) === 1 ? "" : "s"}`;
    case "Play":
      return `Play ${cardName({ Trainer: payload.trainer_card } as Card)}`;
    case "Place": {
      // Tuple variant Place(Card, usize) serializes as a 2-element array.
      const [card, index] = rawPayload as [Card, number];
      return `Place ${cardName(card)} (${slotLabel(index)})`;
    }
    case "Evolve":
      return `Evolve ${at(payload.in_play_idx as number)} into ${cardName(payload.evolution as Card)}`;
    case "UseAbility":
      return `Use Ability (${at(payload.in_play_idx as number)})`;
    case "Attack":
      // Attack(Attack) is a single-field tuple variant, so the payload *is* the Attack struct.
      return `Attack: ${(rawPayload as { title: string }).title}`;
    case "Retreat":
      // Retreat(usize) is a single-field tuple variant — the payload is the bare target index.
      return `Retreat (swap in ${at(rawPayload as number)})`;
    case "Attach": {
      const attachments = payload.attachments as [number, string, number][];
      const targets = attachments.map(([, , idx]) => at(idx)).join(", ");
      const source = payload.is_turn_energy ? " (from Energy Zone)" : "";
      return `Attach Energy${source} → ${targets}`;
    }
    case "MoveEnergy":
      return `Move Energy: ${at(payload.from_in_play_idx as number)} → ${at(payload.to_in_play_idx as number)}`;
    case "AttachTool":
      return `Attach Tool: ${cardName(payload.tool_card as Card)} → ${at(payload.in_play_idx as number)}`;
    case "Heal":
      return `Heal ${payload.amount ?? 0} HP (${at(payload.in_play_idx as number)})`;
    case "HealAndDiscardEnergy":
      return `Heal ${payload.heal_amount ?? 0} HP (${at(payload.in_play_idx as number)})`;
    case "MoveAllDamage":
      return `Move Damage: ${at(payload.from as number)} → ${at(payload.to as number)}`;
    case "Activate":
      return `Switch In: ${at(payload.in_play_idx as number, payload.player as number)}`;
    case "DiscardFossil":
      return `Discard Fossil (${at(payload.in_play_idx as number)})`;
    case "ReturnPokemonToHand":
      return `Return to Hand: ${at(payload.in_play_idx as number)}`;
    case "ShuffleInPlayPokemonIntoDeck":
      return `Shuffle into Deck: ${at(payload.in_play_idx as number)}`;
    case "DiscardToolFromPokemon":
      return `Discard Tool: ${at(payload.in_play_idx as number, payload.player as number)}`;
    case "DiscardOwnBenchedThenDamage":
      return `Discard Benched ${at(payload.in_play_idx as number)}, Deal ${payload.damage} Damage`;
    case "ApplyStatusToOpponentActive":
      return `Apply ${humanizeVariantName(String(payload.condition))} to Opponent's Active`;
    default: {
      const label = humanizeVariantName(variant);
      const context = extractCardContext(payload);
      if (context) return `${label}: ${context}`;
      if (typeof payload.in_play_idx === "number") {
        return `${label} (${at(payload.in_play_idx, (payload.player as number) ?? action.actor)})`;
      }
      return label;
    }
  }
}
