import type { GameOutcome } from "@/lib/gameTypes";

/** Manually force-ends the game — e.g. conceding, or calling a game that's dragged on too long
 * to bother playing out. Undoable like any other step, via the same Undo button. Shared between
 * desktop's header button and the mobile layout's `HamburgerMenu`. */
export default function DeclareWinnerModal({
  onDeclare,
  onClose,
}: {
  onDeclare: (outcome: GameOutcome) => void;
  onClose: () => void;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="flex w-full max-w-sm flex-col gap-4 rounded bg-white p-6 dark:bg-zinc-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-semibold">Declare Winner</h2>
        <p className="text-sm text-zinc-600 dark:text-zinc-400">
          Manually end the game — useful for conceding or calling a game early. You can still
          Undo this afterward.
        </p>
        <div className="flex flex-col gap-2">
          <button
            type="button"
            onClick={() => onDeclare({ Win: 0 })}
            className="rounded border border-zinc-300 px-4 py-2 text-sm hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-800"
          >
            Player 1 wins
          </button>
          <button
            type="button"
            onClick={() => onDeclare({ Win: 1 })}
            className="rounded border border-zinc-300 px-4 py-2 text-sm hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-800"
          >
            Player 2 wins
          </button>
          <button
            type="button"
            onClick={() => onDeclare("Tie")}
            className="rounded border border-zinc-300 px-4 py-2 text-sm hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-800"
          >
            Tie
          </button>
        </div>
        <button type="button" onClick={onClose} className="text-sm text-zinc-500 underline">
          Cancel
        </button>
      </div>
    </div>
  );
}
