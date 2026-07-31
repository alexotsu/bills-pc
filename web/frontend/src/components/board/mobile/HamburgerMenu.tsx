"use client";

import Link from "next/link";
import { useState } from "react";
import DeclareWinnerModal from "@/components/board/DeclareWinnerModal";
import type { GameOutcome } from "@/lib/gameTypes";

/**
 * Bottom-left menu icon covering what desktop puts in its header bar (Undo / Declare Winner /
 * New Game / View saved game) — the mobile layout's top chrome is fully consumed by the
 * opponent's mirrored half, leaving nowhere for persistent header buttons, and these are
 * meaningfully lower-frequency than the moment-to-moment gesture interactions anyway.
 */
export default function HamburgerMenu({
  canUndo,
  onUndo,
  onDeclareWinner,
  onNewGame,
  isGameOver,
  gameId,
}: {
  canUndo: boolean;
  onUndo: () => void;
  onDeclareWinner: (outcome: GameOutcome) => void;
  onNewGame: () => void;
  isGameOver: boolean;
  gameId: string | null;
}) {
  const [open, setOpen] = useState(false);
  const [showDeclareWinner, setShowDeclareWinner] = useState(false);

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        aria-label="Menu"
        className="flex h-10 w-10 items-center justify-center rounded-full bg-zinc-800/80 text-white shadow"
      >
        ☰
      </button>

      {open && (
        <div
          className="fixed inset-0 z-50 flex items-end bg-black/40"
          onClick={() => setOpen(false)}
        >
          <div
            className="flex w-full flex-col gap-2 rounded-t-2xl bg-white p-4 pb-6 dark:bg-zinc-900"
            onClick={(e) => e.stopPropagation()}
          >
            <button
              type="button"
              onClick={() => {
                onUndo();
                setOpen(false);
              }}
              disabled={!canUndo}
              className="rounded border border-zinc-300 px-3 py-2 text-left text-sm disabled:opacity-30 dark:border-zinc-700"
            >
              Undo
            </button>
            <button
              type="button"
              onClick={() => setShowDeclareWinner(true)}
              disabled={isGameOver}
              className="rounded border border-zinc-300 px-3 py-2 text-left text-sm disabled:opacity-30 dark:border-zinc-700"
            >
              Declare Winner
            </button>
            <button
              type="button"
              onClick={() => {
                onNewGame();
                setOpen(false);
              }}
              className="rounded border border-zinc-300 px-3 py-2 text-left text-sm dark:border-zinc-700"
            >
              New Game
            </button>
            {isGameOver && gameId && (
              <Link
                href={`/games/${gameId}`}
                className="rounded border border-zinc-300 px-3 py-2 text-left text-sm underline dark:border-zinc-700"
                onClick={() => setOpen(false)}
              >
                View saved game
              </Link>
            )}
            <button
              type="button"
              onClick={() => setOpen(false)}
              className="mt-1 text-center text-xs text-zinc-500 underline"
            >
              Close
            </button>
          </div>
        </div>
      )}

      {showDeclareWinner && (
        <DeclareWinnerModal
          onDeclare={(outcome) => {
            onDeclareWinner(outcome);
            setShowDeclareWinner(false);
            setOpen(false);
          }}
          onClose={() => setShowDeclareWinner(false)}
        />
      )}
    </>
  );
}
