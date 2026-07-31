// Maps a mobile drag/drop gesture onto one of the engine's own precomputed legal actions
// (`pending.actions`). Mirrors describeAction.ts's approach to SimpleAction's untyped payloads
// (pattern-match by variant name, cast the payload shape per variant) but for matching instead
// of describing. Pure, no React — same reasoning as describeAction.ts for keeping it that way.
//
// Critical invariant, shared with ActionPanel's onSubmit(action): this module never constructs
// an Action. It only ever selects a member of the `actions` array the engine already declared
// legal. A gesture that doesn't match anything resolves to `null`, meaning the caller should
// reject the drop (snap back), never fabricate an action to satisfy the gesture.

import type { Action, Card } from "@/lib/gameTypes";

export type GestureSource =
  | { type: "hand_card"; card: Card; handIndex: number }
  | { type: "energy_zone" }
  | { type: "active_pokemon" };

export type GestureTarget = { type: "slot"; index: number } | { type: "play_zone" };

function cardsEqual(a: Card, b: Card): boolean {
  // No per-instance id exists on a hand card — deep-equal is the only way to recognize "this is
  // the dragged card" in the returned action's payload. With 2 legal copies of the same named
  // card (allowed per deck rules), the engine may expose two structurally-identical legal
  // actions; matching whichever comes first is correct, since they're outcome-identical.
  return JSON.stringify(a) === JSON.stringify(b);
}

function targetsEqual(a: GestureTarget, b: GestureTarget): boolean {
  if (a.type !== b.type) return false;
  return a.type === "slot" && b.type === "slot" ? a.index === b.index : true;
}

function getVariant(action: Action): { variant: string; payload: unknown } | null {
  const simple = action.action;
  if (typeof simple === "string") return { variant: simple, payload: undefined };
  const entries = Object.entries(simple);
  return entries.length === 1 ? { variant: entries[0][0], payload: entries[0][1] } : null;
}

function ownActions(actions: Action[], actor: number): Action[] {
  return actions.filter((a) => a.actor === actor);
}

/**
 * Every (action, target) pair `source` could legally produce. The single place the actual
 * variant-shape matching lives — both `resolveGesture` and `eligibleTargets` are thin views over
 * this, so they can't drift out of sync with each other.
 */
function matchingActions(
  actions: Action[],
  actor: number,
  source: GestureSource,
): { action: Action; target: GestureTarget }[] {
  const results: { action: Action; target: GestureTarget }[] = [];

  for (const action of ownActions(actions, actor)) {
    const parsed = getVariant(action);
    if (!parsed) continue;
    const { variant, payload } = parsed;

    if (source.type === "hand_card") {
      if (variant === "Place") {
        // Place(Card, usize) is a 2-field tuple variant -> serializes as a 2-element array.
        const [card, index] = payload as [Card, number];
        if (cardsEqual(card, source.card)) {
          results.push({ action, target: { type: "slot", index } });
        }
      } else if (variant === "Evolve") {
        const p = payload as { evolution: Card; in_play_idx: number };
        if (cardsEqual(p.evolution, source.card)) {
          results.push({ action, target: { type: "slot", index: p.in_play_idx } });
        }
      } else if (variant === "Play") {
        // Pokémon Tools are played via this same Play action as every other trainer card —
        // there's no separate top-level "AttachTool" to match here. Which Pokémon can receive
        // it is only revealed as a *follow-up* decision after Play is submitted (see
        // attach_tool in src/actions/apply_trainer_action.rs), so a Tool card dragged directly
        // onto a Pokémon needs special handling above this module entirely: it resolves the
        // Play here (against a play_zone target, same as any other trainer card) and then
        // chains the resulting AttachTool choice itself — see MobileBoardLayout's
        // isToolCard/handleDragEnd.
        const p = payload as { trainer_card: unknown };
        const asCard = { Trainer: p.trainer_card } as Card;
        if (cardsEqual(asCard, source.card)) {
          results.push({ action, target: { type: "play_zone" } });
        }
      }
    } else if (source.type === "energy_zone") {
      if (variant === "Attach") {
        const p = payload as { attachments: [number, string, number][]; is_turn_energy: boolean };
        if (p.is_turn_energy) {
          for (const [, , idx] of p.attachments) {
            results.push({ action, target: { type: "slot", index: idx } });
          }
        }
      }
    } else if (source.type === "active_pokemon") {
      if (variant === "Retreat") {
        // Retreat(usize) is a single-field tuple variant -> the payload is the bare target index.
        results.push({ action, target: { type: "slot", index: payload as number } });
      }
    }
  }

  return results;
}

/** Resolves a completed drag (`source` dropped on `target`) to the legal action it represents,
 * or `null` if the drop isn't legal — the caller should snap the drag back rather than submit. */
export function resolveGesture(
  actions: Action[],
  actor: number,
  source: GestureSource,
  target: GestureTarget,
): Action | null {
  const match = matchingActions(actions, actor, source).find((m) => targetsEqual(m.target, target));
  return match?.action ?? null;
}

/** Which drop targets are legal for `source` right now — drives drop-zone highlighting while a
 * drag is in progress, before the user has committed to a specific target. */
export function eligibleTargets(
  actions: Action[],
  actor: number,
  source: GestureSource,
): { slots: Set<number>; playZone: boolean } {
  const slots = new Set<number>();
  let playZone = false;
  for (const { target } of matchingActions(actions, actor, source)) {
    if (target.type === "slot") slots.add(target.index);
    else playZone = true;
  }
  return { slots, playZone };
}

// AttachTool deliberately isn't here: it's never actually a top-level legal action (see the
// "Play" case in matchingActions above), so if it's ever what `pending.actions` contains, that
// means the mobile board's tool-attach chain didn't complete as expected (see
// MobileBoardLayout's handleDragEnd) — that's exactly the case that needs a safety net, not
// another silent exclusion, so it stays reachable via the fallback sheet.
const GESTURE_VARIANTS = new Set(["Place", "Evolve", "Play", "Retreat", "Attach"]);
// Not drag gestures, but still have a dedicated non-fallback UI element elsewhere: Attack opens
// a tap-to-attack sheet, UseAbility and UseStadium are tappable badges, EndTurn has its own
// button.
const OTHER_HANDLED_VARIANTS = new Set(["Attack", "UseAbility", "UseStadium", "EndTurn"]);

/** Legal actions that don't correspond to any gesture, tap target, or dedicated button above —
 * the long tail of one-off card-specific mechanics (`Heal`, `MoveEnergy`, `DiscardFossil`, etc.).
 * Drives the fallback action sheet so nothing is ever unreachable on mobile just because its
 * variant isn't hand-cased into a gesture. */
export function unmappedActions(actions: Action[], actor: number): Action[] {
  return ownActions(actions, actor).filter((action) => {
    const parsed = getVariant(action);
    if (!parsed) return true; // unparseable shape -> surface it rather than silently drop it
    if (GESTURE_VARIANTS.has(parsed.variant) || OTHER_HANDLED_VARIANTS.has(parsed.variant)) {
      // Attach is only gesture-mapped via the energy-zone drag when it's spending the turn's
      // energy-zone attachment; other Attach actions (e.g. ability-granted attachments) aren't
      // reachable by that gesture and still belong in the fallback sheet.
      if (parsed.variant === "Attach") {
        return (parsed.payload as { is_turn_energy: boolean }).is_turn_energy !== true;
      }
      return false;
    }
    return true;
  });
}
