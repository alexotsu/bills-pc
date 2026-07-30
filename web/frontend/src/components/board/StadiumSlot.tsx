import { cardName, type Card } from "@/lib/gameTypes";

export default function StadiumSlot({
  card,
  ownerLabel,
}: {
  card: Card | null;
  ownerLabel?: string;
}) {
  return (
    <div className="flex h-16 w-24 flex-col items-center justify-center rounded border border-dashed border-zinc-300 p-1 text-center text-[10px] dark:border-zinc-700">
      {card ? (
        <>
          <span className="font-medium leading-tight">{cardName(card)}</span>
          {ownerLabel && <span className="text-zinc-500">{ownerLabel}</span>}
        </>
      ) : (
        <span className="text-zinc-400">Stadium</span>
      )}
    </div>
  );
}
