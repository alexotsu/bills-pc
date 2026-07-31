import type { Action } from "@/lib/gameTypes";

/** Renders only when `EndTurn` is actually a legal action right now — never a fabricated
 * action, same discipline as everything else that submits from `pending.actions`. */
export default function EndTurnButton({
  actions,
  onSubmit,
}: {
  actions: Action[];
  onSubmit: (action: Action) => void;
}) {
  const endTurn = actions.find((a) => a.action === "EndTurn");
  if (!endTurn) return null;
  return (
    <button
      type="button"
      onClick={() => onSubmit(endTurn)}
      className="pointer-events-auto rounded-full border-2 border-cyan-300 bg-white px-6 py-2 text-sm font-medium shadow-lg dark:border-cyan-700 dark:bg-zinc-900"
    >
      End Turn
    </button>
  );
}
