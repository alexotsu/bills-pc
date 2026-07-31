import { describe, expect, it } from "vitest";
import { eligibleTargets, resolveGesture, unmappedActions } from "./gestureActions";
import type { Action, Card } from "./gameTypes";

const BULBASAUR: Card = {
  Pokemon: {
    id: "A1-001",
    name: "Bulbasaur",
    stage: 0,
    evolves_from: null,
    hp: 70,
    energy_type: "Grass",
    ability: null,
    attacks: [],
    weakness: null,
    retreat_cost: [],
    rarity: "C",
    booster_pack: "A1",
  },
};

const ROCKY_HELMET: Card = {
  Trainer: {
    id: "A2-148",
    trainer_card_type: "Tool",
    name: "Rocky Helmet",
    effect: "...",
    rarity: "U",
    booster_pack: "A2",
  },
};

const POKE_BALL: Card = {
  Trainer: {
    id: "P-A-005",
    trainer_card_type: "Item",
    name: "Poké Ball",
    effect: "...",
    rarity: "C",
    booster_pack: "P-A",
  },
};

function place(index: number, card: Card = BULBASAUR): Action {
  return { actor: 0, action: { Place: [card, index] }, is_stack: false };
}

function attachTool(index: number, card: Card = ROCKY_HELMET): Action {
  return { actor: 0, action: { AttachTool: { in_play_idx: index, tool_card: card } }, is_stack: false };
}

function play(trainerCard: Card = POKE_BALL): Action {
  return { actor: 0, action: { Play: { trainer_card: (trainerCard as { Trainer: unknown }).Trainer } }, is_stack: false };
}

function attach(attachments: [number, string, number][], isTurnEnergy: boolean): Action {
  return {
    actor: 0,
    action: { Attach: { attachments, is_turn_energy: isTurnEnergy } },
    is_stack: false,
  };
}

function retreat(index: number): Action {
  return { actor: 0, action: { Retreat: index }, is_stack: false };
}

describe("resolveGesture", () => {
  it("matches a hand card dropped on the slot its Place action targets", () => {
    const actions = [place(0), place(2)];
    const result = resolveGesture(
      actions,
      0,
      { type: "hand_card", card: BULBASAUR, handIndex: 0 },
      { type: "slot", index: 2 },
    );
    expect(result).toBe(actions[1]);
  });

  it("returns null when the drop has no matching legal action", () => {
    const actions = [place(0)];
    const result = resolveGesture(
      actions,
      0,
      { type: "hand_card", card: BULBASAUR, handIndex: 0 },
      { type: "slot", index: 1 },
    );
    expect(result).toBeNull();
  });

  it("matches either of two structurally-identical duplicate-card Place actions", () => {
    // Two copies of the same named card in hand -> two indistinguishable legal Place actions
    // for the same target slot. Matching whichever comes first is correct: they're
    // outcome-identical.
    const actions = [place(1), place(1)];
    const result = resolveGesture(
      actions,
      0,
      { type: "hand_card", card: BULBASAUR, handIndex: 0 },
      { type: "slot", index: 1 },
    );
    expect(result).toBe(actions[0]);
  });

  it("matches a tool card dropped on the play zone, via its Play action (not AttachTool)", () => {
    // Pokémon Tools are played via the same top-level Play action as every other trainer card —
    // AttachTool only exists as a follow-up decision *after* Play is submitted, never as
    // something resolveGesture can match directly against a slot target. The mobile board
    // chains Play -> AttachTool itself (see MobileBoardLayout's handleDragEnd); this module only
    // ever needs to get the Play half right.
    const actions = [play(ROCKY_HELMET)];
    const result = resolveGesture(
      actions,
      0,
      { type: "hand_card", card: ROCKY_HELMET, handIndex: 0 },
      { type: "play_zone" },
    );
    expect(result).toBe(actions[0]);
  });

  it("does not match a tool card against an AttachTool action directly", () => {
    // AttachTool should never actually appear as a top-level legal action in practice, but even
    // if it somehow did, a hand_card source has no business matching it directly (see above).
    const result = resolveGesture(
      [attachTool(0)],
      0,
      { type: "hand_card", card: ROCKY_HELMET, handIndex: 0 },
      { type: "slot", index: 0 },
    );
    expect(result).toBeNull();
  });

  it("matches an item card dropped on the play zone", () => {
    const actions = [play()];
    const result = resolveGesture(
      actions,
      0,
      { type: "hand_card", card: POKE_BALL, handIndex: 0 },
      { type: "play_zone" },
    );
    expect(result).toBe(actions[0]);
  });

  it("matches an energy-zone drag against a multi-target Attach action", () => {
    const actions = [attach([[1, "Grass", 0], [1, "Grass", 2]], true)];
    const onActive = resolveGesture(
      actions,
      0,
      { type: "energy_zone" },
      { type: "slot", index: 0 },
    );
    const onBench = resolveGesture(
      actions,
      0,
      { type: "energy_zone" },
      { type: "slot", index: 2 },
    );
    expect(onActive).toBe(actions[0]);
    expect(onBench).toBe(actions[0]);
  });

  it("does not match a non-turn-energy Attach for the energy-zone drag", () => {
    const actions = [attach([[1, "Grass", 0]], false)];
    const result = resolveGesture(
      actions,
      0,
      { type: "energy_zone" },
      { type: "slot", index: 0 },
    );
    expect(result).toBeNull();
  });

  it("matches an active-Pokémon drag against the target Retreat lands on", () => {
    const actions = [retreat(2)];
    const result = resolveGesture(
      actions,
      0,
      { type: "active_pokemon" },
      { type: "slot", index: 2 },
    );
    expect(result).toBe(actions[0]);
  });

  it("rejects a retreat drag to a bench slot with no matching legal action", () => {
    const actions = [retreat(1)];
    const result = resolveGesture(
      actions,
      0,
      { type: "active_pokemon" },
      { type: "slot", index: 3 },
    );
    expect(result).toBeNull();
  });

  it("only matches actions belonging to the given actor", () => {
    const opponentPlace: Action = { actor: 1, action: { Place: [BULBASAUR, 0] }, is_stack: false };
    const result = resolveGesture(
      [opponentPlace],
      0,
      { type: "hand_card", card: BULBASAUR, handIndex: 0 },
      { type: "slot", index: 0 },
    );
    expect(result).toBeNull();
  });
});

describe("eligibleTargets", () => {
  it("collects every legal slot for a hand card that can go to multiple places", () => {
    const actions = [place(0), place(1), place(3)];
    const { slots, playZone } = eligibleTargets(actions, 0, {
      type: "hand_card",
      card: BULBASAUR,
      handIndex: 0,
    });
    expect(slots).toEqual(new Set([0, 1, 3]));
    expect(playZone).toBe(false);
  });

  it("reports the play zone as eligible for a playable item", () => {
    const { slots, playZone } = eligibleTargets([play()], 0, {
      type: "hand_card",
      card: POKE_BALL,
      handIndex: 0,
    });
    expect(slots.size).toBe(0);
    expect(playZone).toBe(true);
  });

  it("returns nothing eligible when there are no matching legal actions", () => {
    const { slots, playZone } = eligibleTargets([], 0, { type: "active_pokemon" });
    expect(slots.size).toBe(0);
    expect(playZone).toBe(false);
  });
});

describe("unmappedActions", () => {
  it("excludes every gesture-mapped and dedicated-control variant", () => {
    const actions: Action[] = [
      place(0),
      play(),
      attach([[1, "Grass", 0]], true),
      retreat(2),
      { actor: 0, action: "EndTurn", is_stack: false },
      { actor: 0, action: { Attack: { title: "Peck", energy_required: [], fixed_damage: 20, effect: null } }, is_stack: false },
      { actor: 0, action: { UseAbility: { in_play_idx: 0 } }, is_stack: false },
      { actor: 0, action: "UseStadium", is_stack: false },
    ];
    expect(unmappedActions(actions, 0)).toEqual([]);
  });

  it("surfaces AttachTool as a safety net, since it's not excluded as gesture-mapped", () => {
    // AttachTool should never actually reach here in the successful case — the mobile board's
    // Play -> AttachTool chain resolves it before it's ever exposed as `pending` (see
    // MobileBoardLayout). If it ever does show up (the chain's chooseNext callback failing to
    // find a match), it needs to stay reachable via the fallback sheet rather than being
    // silently hidden, which is exactly the soft-lock bug this whole design is meant to avoid.
    const action = attachTool(0);
    expect(unmappedActions([action], 0)).toEqual([action]);
  });

  it("surfaces a non-turn-energy Attach (not reachable via the energy-zone drag)", () => {
    const abilityAttach = attach([[1, "Fire", 1]], false);
    expect(unmappedActions([abilityAttach], 0)).toEqual([abilityAttach]);
  });

  it("surfaces card-specific long-tail variants with no gesture or tap mapping", () => {
    const heal: Action = { actor: 0, action: { Heal: { in_play_idx: 0, amount: 20 } }, is_stack: false };
    expect(unmappedActions([heal], 0)).toEqual([heal]);
  });

  it("only surfaces the given actor's own unmapped actions", () => {
    const heal: Action = { actor: 1, action: { Heal: { in_play_idx: 0, amount: 20 } }, is_stack: false };
    expect(unmappedActions([heal], 0)).toEqual([]);
  });
});
