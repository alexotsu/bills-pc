"use client";

import { useDraggable } from "@dnd-kit/core";
import type { CSSProperties } from "react";
import type { PlayedCard } from "@/lib/gameTypes";
import DroppableSlot from "./DroppableSlot";

/**
 * The acting player's own active-Pokémon slot: draggable (down onto a bench slot) when a legal
 * `Retreat` exists, tappable to open the attack sheet, and still a drop target for anything
 * targeting slot 0 (Place/Evolve/AttachTool/energy). dnd-kit's activation-constraint sensors
 * (configured in `MobileBoardLayout`) are what let the same element be both a drag source and a
 * tap target — a short press that doesn't exceed the distance/delay threshold fires as a normal
 * click, only a real drag exceeds it.
 */
export default function DraggableActivePokemon({
  played,
  isValidTarget,
  canRetreat,
  onTapAttack,
  onUseAbility,
}: {
  played: PlayedCard | null;
  isValidTarget: boolean;
  canRetreat: boolean;
  onTapAttack: () => void;
  onUseAbility?: () => void;
}) {
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: "active",
    data: { type: "active_pokemon" },
    disabled: !canRetreat,
  });

  const style: CSSProperties = {
    touchAction: "none",
    ...(transform && {
      transform: `translate3d(${transform.x}px, ${transform.y}px, 0)`,
      opacity: isDragging ? 0.4 : 1,
      zIndex: isDragging ? 10 : undefined,
    }),
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      {...listeners}
      {...attributes}
      onClick={played ? onTapAttack : undefined}
    >
      <DroppableSlot
        index={0}
        played={played}
        label="Active"
        isValidTarget={isValidTarget}
        onUseAbility={onUseAbility}
      />
    </div>
  );
}
