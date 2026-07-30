import { cardName, type Card } from "@/lib/gameTypes";

export default function DiscardPile({ cards }: { cards: Card[] }) {
  const top = cards[cards.length - 1];
  return (
    <div className="flex shrink-0 flex-col items-center gap-1 text-[10px] text-zinc-500">
      <span>Discard ({cards.length})</span>
      <div className="flex h-16 w-12 items-center justify-center rounded border border-zinc-300 p-0.5 text-center text-[9px] leading-tight dark:border-zinc-700">
        {top ? cardName(top) : "Empty"}
      </div>
    </div>
  );
}
