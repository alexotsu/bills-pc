import { cardName, type Card } from "@/lib/gameTypes";

export type CardFaceSize = "md" | "sm";

const SIZE_CLASSES: Record<CardFaceSize, string> = {
  md: "h-20 w-14 text-[9px]",
  // Used by the mobile hand row, where a hand of up to 10 cards has to fit (wrapped across at
  // most 2 rows) without pushing the rest of the board off-screen.
  sm: "h-14 w-10 text-[7px]",
};

/** The small card visual used everywhere a hand card is rendered — desktop's `HandRow` and the
 * mobile hand row both draw from this single implementation rather than each having their own
 * copy. `revealed=false` shows a face-down back only (no name), the hidden-hand default for
 * whichever seat isn't the one currently acting (see `web/SPEC.md`). */
export default function CardFace({
  card,
  revealed,
  size = "md",
}: {
  card: Card;
  revealed: boolean;
  size?: CardFaceSize;
}) {
  const dims = SIZE_CLASSES[size];
  if (!revealed) {
    return (
      <div
        className={`shrink-0 rounded border border-zinc-400 bg-zinc-200 dark:border-zinc-600 dark:bg-zinc-800 ${dims}`}
      />
    );
  }
  return (
    <div
      className={`flex shrink-0 items-center justify-center rounded border border-zinc-300 p-1 text-center leading-tight dark:border-zinc-700 ${dims}`}
    >
      {cardName(card)}
    </div>
  );
}
