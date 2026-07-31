"use client";

import type { Action } from "@/lib/gameTypes";

/** Opened by tapping the active Pokémon whenever it has 1+ legal `Attack` actions — always shows
 * the sheet even for exactly one, rather than auto-submitting, since a slightly-off drag-start
 * can register as a tap and an accidental one-tap attack would be hard to undo cleanly mid-turn. */
export default function AttackSheet({
  actions,
  onSubmit,
  onClose,
}: {
  actions: Action[];
  onSubmit: (action: Action) => void;
  onClose: () => void;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-end bg-black/40"
      onClick={onClose}
    >
      <div
        className="w-full rounded-t-2xl bg-white p-4 pb-6 dark:bg-zinc-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="mb-3 text-sm font-semibold">Attack</h2>
        <div className="flex flex-col gap-2">
          {actions.map((action, i) => {
            // Attack(Attack) is a single-field tuple variant -> the payload *is* the Attack struct.
            const attack = (action.action as { Attack: { title: string; fixed_damage: number } })
              .Attack;
            return (
              <button
                key={i}
                type="button"
                onClick={() => onSubmit(action)}
                className="flex items-center justify-between rounded border border-zinc-300 px-3 py-2 text-sm dark:border-zinc-700"
              >
                <span>{attack.title}</span>
                {attack.fixed_damage > 0 && <span>{attack.fixed_damage}</span>}
              </button>
            );
          })}
        </div>
        <button
          type="button"
          onClick={onClose}
          className="mt-3 w-full text-center text-xs text-zinc-500 underline"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
