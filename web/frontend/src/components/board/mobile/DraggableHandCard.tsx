"use client";

import { useDraggable } from "@dnd-kit/core";
import type { CSSProperties } from "react";
import type { Card } from "@/lib/gameTypes";
import CardFace from "../CardFace";

export default function DraggableHandCard({ card, handIndex }: { card: Card; handIndex: number }) {
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: `hand:${handIndex}`,
    data: { type: "hand_card", card, handIndex },
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
    // shrink-0 here specifically (not just on CardFace inside it): this div, not CardFace, is
    // the actual flex item MobileHandRow's flex-wrap container manages — without shrink-0 at
    // this level it was free to shrink under flex's default behavior, which is what made hand
    // cards on the acting player's side render smaller than the opponent's (non-draggable, so
    // CardFace itself was the direct flex item and already had its own shrink-0).
    <div ref={setNodeRef} style={style} {...listeners} {...attributes} className="shrink-0">
      <CardFace card={card} revealed size="sm" />
    </div>
  );
}
