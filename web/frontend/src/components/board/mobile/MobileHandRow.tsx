import type { Card } from "@/lib/gameTypes";
import CardFace from "../CardFace";
import DraggableHandCard from "./DraggableHandCard";

/** Unlike desktop's `HandRow`, this wraps instead of horizontally scrolling — dodges the
 * drag-vs-scroll gesture conflict entirely rather than fighting it with sensor tuning. Hand size
 * can reach 10, so cards render at `CardFace`'s "sm" size here (both the draggable and
 * non-draggable/opponent path — using the same size on both is also what keeps them visually
 * consistent, see `DraggableHandCard`) to keep a full hand's footprint from pushing the rest of
 * the board off-screen even when wrapped across 2 rows. */
export default function MobileHandRow({
  cards,
  revealed,
  draggable,
}: {
  cards: Card[];
  revealed: boolean;
  draggable: boolean;
}) {
  if (cards.length === 0) {
    return <div className="flex h-14 items-center justify-center text-xs text-zinc-400">Hand is empty</div>;
  }
  return (
    <div className="flex flex-wrap justify-center gap-1">
      {cards.map((card, i) =>
        draggable && revealed ? (
          <DraggableHandCard key={i} card={card} handIndex={i} />
        ) : (
          <CardFace key={i} card={card} revealed={revealed} size="sm" />
        ),
      )}
    </div>
  );
}
