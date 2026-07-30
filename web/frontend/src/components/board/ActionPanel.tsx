"use client";

import { useState } from "react";
import { describeAction } from "@/lib/describeAction";
import {
  cardName,
  type Action,
  type Card,
  type GameOutcome,
  type PendingDecision,
  type State,
} from "@/lib/gameTypes";
import { useNumberKeySelection } from "@/hooks/useNumberKeySelection";

// Matches ITEMS_PER_PAGE in src/tui/app.rs, for the same reason: legal-action (and draw-choice)
// lists can exceed what's comfortably clickable on one screen.
const ITEMS_PER_PAGE = 9;

function paginate<T>(items: T[], page: number) {
  const totalPages = Math.max(1, Math.ceil(items.length / ITEMS_PER_PAGE));
  const currentPage = Math.min(page, totalPages - 1);
  const start = currentPage * ITEMS_PER_PAGE;
  return { pageItems: items.slice(start, start + ITEMS_PER_PAGE), currentPage, totalPages, start };
}

function Pager({
  page,
  totalPages,
  onChange,
}: {
  page: number;
  totalPages: number;
  onChange: (page: number) => void;
}) {
  if (totalPages <= 1) return null;
  return (
    <div className="flex items-center gap-2 text-xs text-zinc-500">
      <button
        type="button"
        onClick={() => onChange(page - 1)}
        disabled={page === 0}
        className="rounded border border-zinc-300 px-2 py-0.5 disabled:opacity-30 dark:border-zinc-700"
      >
        Prev
      </button>
      <span>
        Page {page + 1} / {totalPages}
      </span>
      <button
        type="button"
        onClick={() => onChange(page + 1)}
        disabled={page >= totalPages - 1}
        className="rounded border border-zinc-300 px-2 py-0.5 disabled:opacity-30 dark:border-zinc-700"
      >
        Next
      </button>
    </div>
  );
}

function ActionList({
  actions,
  state,
  page,
  onPageChange,
  onSubmit,
}: {
  actions: Action[];
  state: State;
  page: number;
  onPageChange: (page: number) => void;
  onSubmit: (action: Action) => void;
}) {
  const { pageItems, currentPage, totalPages, start } = paginate(actions, page);
  useNumberKeySelection(pageItems, onSubmit);
  return (
    <div className="flex flex-col gap-2">
      <ul className="flex flex-col gap-1">
        {pageItems.map((action, i) => (
          <li key={start + i}>
            <button
              type="button"
              onClick={() => onSubmit(action)}
              className="w-full rounded border border-zinc-300 px-3 py-1.5 text-left text-sm hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-900"
            >
              {start + i + 1}. {describeAction(action, state)}
            </button>
          </li>
        ))}
      </ul>
      <Pager page={currentPage} totalPages={totalPages} onChange={onPageChange} />
    </div>
  );
}

/** For `awaiting_draw`, the candidate cards aren't in `PendingDecision` itself — mirroring the
 * TUI's `maybe_offer_draw_choice` (`src/tui/app.rs`), they're derived here from
 * `state.decks[actor].cards`, the full remaining deck in draw order. */
function DrawChoice({
  actor,
  state,
  page,
  onPageChange,
  onSubmit,
}: {
  actor: number;
  state: State;
  page: number;
  onPageChange: (page: number) => void;
  onSubmit: (card: Card | null) => void;
}) {
  const candidates = state.decks[actor].cards;
  const { pageItems, currentPage, totalPages, start } = paginate(candidates, page);
  useNumberKeySelection(pageItems, onSubmit);
  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        onClick={() => onSubmit(null)}
        className="w-full rounded bg-foreground px-3 py-1.5 text-left text-sm text-background"
      >
        Draw normally (top of deck)
      </button>
      {candidates.length > 0 && (
        <>
          <p className="text-xs text-zinc-500">Or choose a specific card:</p>
          <ul className="flex flex-col gap-1">
            {pageItems.map((card, i) => (
              <li key={start + i}>
                <button
                  type="button"
                  onClick={() => onSubmit(card)}
                  className="w-full rounded border border-zinc-300 px-3 py-1.5 text-left text-sm hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-900"
                >
                  {start + i + 1}. {cardName(card)}
                </button>
              </li>
            ))}
          </ul>
          <Pager page={currentPage} totalPages={totalPages} onChange={onPageChange} />
        </>
      )}
    </div>
  );
}

function GameOverMessage({ outcome }: { outcome: GameOutcome | null }) {
  if (!outcome) return <p className="text-sm">The game ended with no winner.</p>;
  if (outcome === "Tie") return <p className="text-sm font-semibold">It&apos;s a tie!</p>;
  return <p className="text-sm font-semibold">Player {outcome.Win + 1} wins!</p>;
}

export default function ActionPanel({
  pending,
  state,
  onSubmitAction,
  onSubmitDraw,
  canUndo,
  onUndo,
}: {
  pending: PendingDecision;
  state: State;
  onSubmitAction: (action: Action) => void;
  onSubmitDraw: (card: Card | null) => void;
  canUndo: boolean;
  onUndo: () => void;
}) {
  const [page, setPage] = useState(0);
  // Resets to page 1 on every new decision (and after undo, since that produces a "new"
  // decision too) — matches App.action_page's reset points in src/tui/app.rs. Comparing and
  // re-seeding state during the render body itself (rather than in a useEffect) is the pattern
  // react.dev recommends for "reset state when a prop changes" — see the identical pattern (and
  // fuller explanation) in DeckBuilder.tsx.
  const [pageResetFor, setPageResetFor] = useState(pending);
  if (pending !== pageResetFor) {
    setPageResetFor(pending);
    setPage(0);
  }

  return (
    <div className="flex flex-col gap-3 rounded border border-zinc-200 p-3 dark:border-zinc-800">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold">
          {pending.kind === "game_over" ? "Game Over" : `Player ${pending.actor + 1}'s decision`}
        </h2>
        <button
          type="button"
          onClick={onUndo}
          disabled={!canUndo}
          className="rounded border border-zinc-300 px-2 py-1 text-xs disabled:opacity-30 dark:border-zinc-700"
        >
          Undo
        </button>
      </div>

      {pending.kind === "awaiting_action" && (
        <ActionList
          actions={pending.actions}
          state={state}
          page={page}
          onPageChange={setPage}
          onSubmit={onSubmitAction}
        />
      )}
      {pending.kind === "awaiting_draw" && (
        <DrawChoice
          actor={pending.actor}
          state={state}
          page={page}
          onPageChange={setPage}
          onSubmit={onSubmitDraw}
        />
      )}
      {pending.kind === "game_over" && <GameOverMessage outcome={pending.outcome} />}
    </div>
  );
}
