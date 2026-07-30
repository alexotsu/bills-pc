import { cardName, type Card } from "@/lib/gameTypes";

/** `revealed=false` shows face-down backs only (card count still visible) — the hidden-hand
 * default for whichever seat isn't the one currently acting, per web/SPEC.md. */
export default function HandRow({ cards, revealed }: { cards: Card[]; revealed: boolean }) {
  if (cards.length === 0) {
    return <p className="text-xs text-zinc-400">Hand is empty</p>;
  }
  return (
    <div className="flex gap-1 overflow-x-auto pb-1">
      {cards.map((card, i) =>
        revealed ? (
          <div
            key={i}
            className="flex h-20 w-14 shrink-0 items-center justify-center rounded border border-zinc-300 p-1 text-center text-[9px] leading-tight dark:border-zinc-700"
          >
            {cardName(card)}
          </div>
        ) : (
          <div
            key={i}
            className="h-20 w-14 shrink-0 rounded border border-zinc-400 bg-zinc-200 dark:border-zinc-600 dark:bg-zinc-800"
          />
        ),
      )}
    </div>
  );
}
