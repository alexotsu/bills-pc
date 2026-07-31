"use client";

import { useDroppable } from "@dnd-kit/core";
import type { PlayedCard } from "@/lib/gameTypes";
import PokemonSlot from "../PokemonSlot";
import AbilityBadge from "./AbilityBadge";

/** Wraps the existing (desktop) `PokemonSlot` display with a drop target for slot `index` on the
 * acting player's own board. `isValidTarget` comes from `eligibleTargets()` in
 * `gestureActions.ts`, computed once per drag by `MobileBoardLayout` — dnd-kit itself only knows
 * *a* drag is active, not which slots are legal for it under game rules. `onUseAbility` renders a
 * tappable badge when this slot has a legal `UseAbility` action — abilities aren't active-only,
 * so bench slots need this too (unlike the drag-to-retreat/tap-to-attack behaviors, which are
 * active-slot-specific and live on `DraggableActivePokemon` instead). */
export default function DroppableSlot({
  index,
  played,
  label,
  isValidTarget,
  onUseAbility,
}: {
  index: number;
  played: PlayedCard | null;
  label: string;
  isValidTarget: boolean;
  onUseAbility?: () => void;
}) {
  const { setNodeRef, isOver } = useDroppable({
    id: `slot:${index}`,
    data: { type: "slot", index },
  });

  return (
    <div
      ref={setNodeRef}
      className={`relative rounded transition-shadow ${
        isValidTarget
          ? isOver
            ? "ring-2 ring-emerald-400"
            : "ring-2 ring-emerald-200 dark:ring-emerald-800"
          : ""
      }`}
    >
      <PokemonSlot played={played} label={label} />
      {onUseAbility && <AbilityBadge onTap={onUseAbility} />}
    </div>
  );
}
