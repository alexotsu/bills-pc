"use client";

import { useDraggable } from "@dnd-kit/core";
import type { CSSProperties } from "react";
import type { EnergyZoneState } from "@/lib/gameTypes";
import EnergyDot from "../EnergyDot";

/** Combined current+next energy indicator (both `EnergyZoneState` fields, unlike desktop's
 * separate `EnergyZone`). On the acting player's own side, the current-energy dot is also the
 * drag source for attaching it to a Pokémon — disabled (and non-draggable) when there's nothing
 * to attach or it isn't this player's turn to drag from. */
export default function EnergyWidget({
  zone,
  draggable,
}: {
  zone: EnergyZoneState;
  draggable: boolean;
}) {
  const canDrag = draggable && zone.current !== null;
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: "energy-zone",
    data: { type: "energy_zone" },
    disabled: !canDrag,
  });

  const style: CSSProperties | undefined = canDrag
    ? {
        touchAction: "none",
        ...(transform && {
          transform: `translate3d(${transform.x}px, ${transform.y}px, 0)`,
          opacity: isDragging ? 0.4 : 1,
        }),
      }
    : undefined;

  return (
    <div
      ref={canDrag ? setNodeRef : undefined}
      style={style}
      {...(canDrag ? { ...listeners, ...attributes } : {})}
      className="relative flex h-10 w-10 items-center justify-center"
    >
      {zone.current ? (
        <EnergyDot type={zone.current} title={`Available: ${zone.current}`} />
      ) : (
        <span className="h-3 w-3 rounded-full border border-dashed border-zinc-400" />
      )}
      {zone.next && (
        <span className="absolute -bottom-1 -right-1">
          <EnergyDot type={zone.next} title={`Next: ${zone.next}`} />
        </span>
      )}
    </div>
  );
}
