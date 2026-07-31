import { cardName, type Card } from "@/lib/gameTypes";

/** Deck-back stack with remaining count, plus the discard pile peeking out from directly behind
 * it — the real app's own convention for showing both without a dedicated row (matches the
 * reference screenshot; desktop's separate `DiscardPile` gets its own row instead). */
export default function DeckPile({
  deckCards,
  discardCards,
}: {
  deckCards: Card[];
  discardCards: Card[];
}) {
  const topDiscard = discardCards[discardCards.length - 1];
  return (
    <div className="flex shrink-0 flex-col items-center gap-1 text-[10px] text-zinc-500">
      <div className="relative h-16 w-12">
        {topDiscard && (
          <div
            title={`Discard (${discardCards.length}): ${cardName(topDiscard)}`}
            className="absolute left-1.5 top-1.5 h-16 w-12 rounded border border-zinc-300 bg-zinc-100 dark:border-zinc-700 dark:bg-zinc-900"
          />
        )}
        <div className="absolute inset-0 flex items-center justify-center rounded border border-zinc-400 bg-zinc-300 text-[10px] font-medium text-zinc-700 dark:border-zinc-600 dark:bg-zinc-700 dark:text-zinc-200">
          {deckCards.length}
        </div>
      </div>
      <span>Deck</span>
    </div>
  );
}
