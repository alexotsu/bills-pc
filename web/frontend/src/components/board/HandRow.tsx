import type { Card } from "@/lib/gameTypes";
import CardFace from "./CardFace";

/** `revealed=false` shows face-down backs only (card count still visible) — the hidden-hand
 * default for whichever seat isn't the one currently acting, per web/SPEC.md. */
export default function HandRow({ cards, revealed }: { cards: Card[]; revealed: boolean }) {
  if (cards.length === 0) {
    return <p className="text-xs text-zinc-400">Hand is empty</p>;
  }
  return (
    <div className="flex gap-1 overflow-x-auto pb-1">
      {cards.map((card, i) => (
        <CardFace key={i} card={card} revealed={revealed} />
      ))}
    </div>
  );
}
