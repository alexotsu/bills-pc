"use client";

import {
  closestCenter,
  DndContext,
  DragOverlay,
  pointerWithin,
  PointerSensor,
  TouchSensor,
  useSensor,
  useSensors,
  type CollisionDetection,
  type DragEndEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { useMemo, useState } from "react";
import {
  eligibleTargets,
  resolveGesture,
  unmappedActions,
  type GestureSource,
  type GestureTarget,
} from "@/lib/gestureActions";
import {
  isPokemonCard,
  type Action,
  type Card,
  type GameOutcome,
  type PendingDecision,
  type State,
} from "@/lib/gameTypes";
import CardFace from "../CardFace";
import AttackSheet from "./AttackSheet";
import EndTurnButton from "./EndTurnButton";
import FallbackActionSheet from "./FallbackActionSheet";
import HamburgerMenu from "./HamburgerMenu";
import MobilePlayerHalf from "./MobilePlayerHalf";
import PlayZoneOverlay from "./PlayZoneOverlay";
import TurnChangeFlash from "./TurnChangeFlash";

function actionsWithVariant(actions: Action[], actor: number, variant: string): Action[] {
  return actions.filter(
    (a) => a.actor === actor && typeof a.action === "object" && a.action !== null && variant in a.action,
  );
}

/** Pokémon Tools are played via the same top-level `Play` action as every other trainer card —
 * `AttachTool` (choosing which Pokémon receives it) only appears as a *follow-up* decision after
 * `Play` is submitted (see `attach_tool` in `src/actions/apply_trainer_action.rs`), never as a
 * legal action on its own. This is why tool-card gestures need special handling everywhere below
 * instead of going through the generic single-step `resolveGesture` path. */
function isToolCard(card: Card): boolean {
  return !isPokemonCard(card) && card.Trainer.trainer_card_type === "Tool";
}

function DragPreview({ source }: { source: GestureSource }) {
  if (source.type === "hand_card") return <CardFace card={source.card} revealed />;
  if (source.type === "energy_zone") {
    return <div className="h-10 w-10 rounded-full bg-white/90 shadow-lg dark:bg-zinc-800/90" />;
  }
  return (
    <div className="h-40 w-28 rounded border-2 border-emerald-400 bg-white/80 shadow-lg dark:bg-zinc-900/80" />
  );
}

/**
 * Top-level mobile game board: owns the `DndContext`, sensors, collision detection, and drag
 * state, and composes every other mobile-only component. Wraps only the acting player's own
 * half plus the shared central band — every gesture-mappable action (`Place`, `Evolve`,
 * `AttachTool`, `Attach` with `is_turn_energy`, `Retreat`, `Play`) targets only the actor's own
 * board (confirmed from the action payload shapes in `src/actions/types.rs`), so the opponent's
 * mirrored half never needs drag/drop wiring — it renders the existing desktop display
 * components read-only, same as `BoardLayout`'s `PlayerRow` does.
 */
export default function MobileBoardLayout({
  pending,
  state,
  canUndo,
  gameId,
  submitAction,
  submitActionThenChoose,
  submitDraw,
  undo,
  declareWinner,
  onRestart,
}: {
  pending: PendingDecision;
  state: State;
  canUndo: boolean;
  gameId: string | null;
  submitAction: (action: Action) => PendingDecision | undefined;
  submitActionThenChoose: (
    first: Action,
    chooseNext: (decision: PendingDecision) => Action | null,
  ) => PendingDecision | undefined;
  submitDraw: (card: Card | null) => void;
  undo: () => void;
  declareWinner: (outcome: GameOutcome) => void;
  onRestart: () => void;
}) {
  const [dragSource, setDragSource] = useState<GestureSource | null>(null);
  const [attackSheetOpen, setAttackSheetOpen] = useState(false);

  // Same convention as desktop's GameBoard: whoever's currently acting renders at the bottom
  // with their hand revealed; on game_over there's no "acting" seat left, so fall back to
  // whoever the engine left as current_player and reveal both hands.
  const bottomSeat = pending.kind === "game_over" ? state.current_player : pending.actor;
  const topSeat = 1 - bottomSeat;
  const handRevealed: [boolean, boolean] =
    pending.kind === "game_over" ? [true, true] : [pending.actor === 0, pending.actor === 1];

  // Wrapped in useMemo (not a plain conditional) so the `[]` fallback is a stable reference
  // across renders when not awaiting_action, rather than a new array every render — otherwise
  // every useMemo below that depends on `actions` would recompute on every render regardless of
  // whether anything actually changed.
  const actions = useMemo(
    () => (pending.kind === "awaiting_action" ? pending.actions : []),
    [pending],
  );
  const canRetreat = actionsWithVariant(actions, bottomSeat, "Retreat").length > 0;
  const attackActions = actionsWithVariant(actions, bottomSeat, "Attack");
  // UseStadium is a unit variant (bare string, e.g. "UseStadium"), not an object payload, so
  // actionsWithVariant (which only handles `{ Variant: payload }` shapes) doesn't apply here.
  const useStadiumAction = actions.find((a) => a.actor === bottomSeat && a.action === "UseStadium");
  const abilitySlots = useMemo(
    () =>
      new Set(
        actionsWithVariant(actions, bottomSeat, "UseAbility").map(
          (a) => (a.action as { UseAbility: { in_play_idx: number } }).UseAbility.in_play_idx,
        ),
      ),
    [actions, bottomSeat],
  );
  const unmapped = useMemo(() => unmappedActions(actions, bottomSeat), [actions, bottomSeat]);

  const eligible = useMemo(() => {
    if (!dragSource) return { slots: new Set<number>(), playZone: false };
    if (dragSource.type === "hand_card" && isToolCard(dragSource.card)) {
      // AttachTool isn't a real legal action yet at this point (see isToolCard's doc comment),
      // so there's nothing in `actions` for eligibleTargets to find. Eligibility instead comes
      // straight from board state, mirroring the engine's own attachment rule
      // (enumerate_tool_choices in src/tools.rs: any Pokémon, no type/stage restriction, at
      // most one tool each).
      const slots = new Set<number>();
      state.in_play_pokemon[bottomSeat].forEach((played, i) => {
        if (played && !played.attached_tool) slots.add(i);
      });
      return { slots, playZone: false };
    }
    return eligibleTargets(actions, bottomSeat, dragSource);
  }, [actions, bottomSeat, dragSource, state]);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(TouchSensor, { activationConstraint: { delay: 150, tolerance: 8 } }),
  );

  // Restricted to droppables that are actually legal for the current drag source *before*
  // running any geometric collision test — e.g. a dragged Tool card is only ever eligible for
  // in-play slots, so "play-zone" (which registers a large rect, always, for stable measurement)
  // is excluded outright rather than relying on geometry/sort-order to avoid it. This is what
  // fixed a real bug: dropping a Tool directly onto a Pokémon previously sometimes resolved to
  // the play zone instead, which has no legal AttachTool-via-play-zone match, so nothing was
  // submitted and the tool appeared to "vanish" mid-drag with no valid action taken.
  //
  // Within that restricted set, pointerWithin runs first (strict — correct for the small,
  // adjacent in-play slots), falling back to closestCenter for a forgiving landing near a slot's
  // edge. resolveGesture in handleDragEnd still independently re-validates legality regardless,
  // so this is a UX/robustness improvement, not the only thing standing between a gesture and an
  // illegal action.
  const collisionDetection: CollisionDetection = (args) => {
    const eligibleIds = new Set<string>([...eligible.slots].map((i) => `slot:${i}`));
    if (eligible.playZone) eligibleIds.add("play-zone");
    const restricted = {
      ...args,
      droppableContainers: args.droppableContainers.filter((c) => eligibleIds.has(String(c.id))),
    };
    const pointerHits = pointerWithin(restricted);
    return pointerHits.length > 0 ? pointerHits : closestCenter(restricted);
  };

  function handleDragStart(event: DragStartEvent) {
    setDragSource((event.active.data.current as GestureSource | undefined) ?? null);
  }

  function handleDragEnd(event: DragEndEvent) {
    setDragSource(null);
    const source = event.active.data.current as GestureSource | undefined;
    const overData = event.over?.data.current as { type: string; index?: number } | undefined;
    if (!source || !overData) return;
    const target: GestureTarget =
      overData.type === "slot" ? { type: "slot", index: overData.index as number } : { type: "play_zone" };

    if (source.type === "hand_card" && isToolCard(source.card)) {
      // Dropping a Tool anywhere but directly on a Pokémon is a no-op now — there's no longer a
      // "drop in the middle" step to fall back to (that used to leave the player stranded: it
      // submitted Play with no way to complete the follow-up AttachTool choice it triggers).
      if (target.type !== "slot") return;
      const playAction = resolveGesture(actions, bottomSeat, source, { type: "play_zone" });
      if (!playAction) return;
      submitActionThenChoose(playAction, (decision) => {
        if (decision.kind !== "awaiting_action") return null;
        return (
          decision.actions.find(
            (a) =>
              a.actor === bottomSeat &&
              typeof a.action === "object" &&
              a.action !== null &&
              "AttachTool" in a.action &&
              (a.action as { AttachTool: { in_play_idx: number } }).AttachTool.in_play_idx ===
                target.index,
          ) ?? null
        );
      });
      return;
    }

    const action = resolveGesture(actions, bottomSeat, source, target);
    if (action) submitAction(action);
  }

  function handleTapAttack() {
    if (attackActions.length > 0) setAttackSheetOpen(true);
  }

  function handleUseAbility(inPlayIdx: number) {
    const action = actionsWithVariant(actions, bottomSeat, "UseAbility").find(
      (a) => (a.action as { UseAbility: { in_play_idx: number } }).UseAbility.in_play_idx === inPlayIdx,
    );
    if (action) submitAction(action);
  }

  function handleUseStadium() {
    if (useStadiumAction) submitAction(useStadiumAction);
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={collisionDetection}
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
      onDragCancel={() => setDragSource(null)}
    >
      <div className="relative flex h-dvh flex-col overflow-hidden bg-gradient-to-b from-indigo-100 to-cyan-100 dark:from-zinc-900 dark:to-zinc-950">
        <TurnChangeFlash state={state} />

        <div className="flex flex-1 flex-col justify-between overflow-y-auto">
          <MobilePlayerHalf
            seat={topSeat}
            state={state}
            mirrored
            handRevealed={handRevealed[topSeat]}
            eligibleSlots={new Set()}
            canRetreat={false}
            abilitySlots={new Set()}
            canUseStadium={!!useStadiumAction}
            onTapAttack={() => {}}
            onUseAbility={() => {}}
            onUseStadium={handleUseStadium}
          />

          <div className="relative flex items-center justify-center py-2">
            <PlayZoneOverlay visible={eligible.playZone} />
            <EndTurnButton actions={actions} onSubmit={submitAction} />
          </div>

          <MobilePlayerHalf
            seat={bottomSeat}
            state={state}
            mirrored={false}
            handRevealed={handRevealed[bottomSeat]}
            eligibleSlots={eligible.slots}
            canRetreat={canRetreat}
            abilitySlots={abilitySlots}
            canUseStadium={!!useStadiumAction}
            onTapAttack={handleTapAttack}
            onUseAbility={handleUseAbility}
            onUseStadium={handleUseStadium}
          />
        </div>

        <div className="fixed bottom-4 left-4 z-40">
          <HamburgerMenu
            canUndo={canUndo}
            onUndo={undo}
            onDeclareWinner={declareWinner}
            onNewGame={onRestart}
            isGameOver={pending.kind === "game_over"}
            gameId={gameId}
          />
        </div>

        {attackSheetOpen && (
          <AttackSheet
            actions={attackActions}
            onSubmit={(action) => {
              submitAction(action);
              setAttackSheetOpen(false);
            }}
            onClose={() => setAttackSheetOpen(false)}
          />
        )}

        <FallbackActionSheet
          unmapped={unmapped}
          state={state}
          onSubmitAction={submitAction}
          drawChoice={
            pending.kind === "awaiting_draw"
              ? { actor: pending.actor, candidates: state.decks[pending.actor].cards, onSubmit: submitDraw }
              : null
          }
          gameOverOutcome={pending.kind === "game_over" ? pending.outcome : undefined}
        />

        <DragOverlay>{dragSource && <DragPreview source={dragSource} />}</DragOverlay>
      </div>
    </DndContext>
  );
}
