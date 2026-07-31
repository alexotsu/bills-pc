"use client";

import { useDroppable } from "@dnd-kit/core";

/** Drop target for Items/Supporters/Stadiums, covering the middle 50% of the *screen* — `fixed`
 * (not `absolute`), since its container is a small row sandwiched between the two player halves,
 * not full-screen; positioning it `absolute` against that row was a real bug (it ended up
 * confined to that row's own small height instead of the intended large, easy-to-hit target).
 * Always registered (so its rect is stable for collision detection) but only visually shown
 * while an eligible card is being dragged — otherwise it's normal empty space behind the End
 * Turn button, per the "zones are highlighted [during a drag]" framing. `pointer-events-none` so
 * it never intercepts taps on the End Turn button when idle. */
export default function PlayZoneOverlay({ visible }: { visible: boolean }) {
  const { setNodeRef, isOver } = useDroppable({ id: "play-zone", data: { type: "play_zone" } });

  return (
    <div
      ref={setNodeRef}
      className={`pointer-events-none fixed inset-[25%] z-30 rounded-2xl border-2 border-dashed transition-opacity ${
        visible ? "opacity-100" : "opacity-0"
      } ${isOver ? "border-emerald-400 bg-emerald-400/20" : "border-emerald-200 bg-emerald-200/10 dark:border-emerald-800"}`}
    />
  );
}
