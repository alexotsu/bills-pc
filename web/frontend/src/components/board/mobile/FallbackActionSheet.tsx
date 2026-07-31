"use client";

import { describeAction } from "@/lib/describeAction";
import { cardName, type Action, type Card, type GameOutcome, type State } from "@/lib/gameTypes";

/**
 * Covers everything gestures can't express, in one bottom sheet:
 * - `unmapped` legal actions (the long tail of one-off card-specific mechanics with no gesture,
 *   tap target, or dedicated button — see `gestureActions.ts`'s `unmappedActions`), auto-open
 *   whenever there are any.
 * - The draw-choice decision (`awaiting_draw`) — not gesture-reachable at all, so always shown
 *   when pending on this.
 * - The game-over message.
 */
export default function FallbackActionSheet({
  unmapped,
  state,
  onSubmitAction,
  drawChoice,
  gameOverOutcome,
}: {
  unmapped: Action[];
  state: State;
  onSubmitAction: (action: Action) => void;
  drawChoice: { actor: number; candidates: Card[]; onSubmit: (card: Card | null) => void } | null;
  gameOverOutcome: GameOutcome | null | undefined;
}) {
  if (unmapped.length === 0 && !drawChoice && gameOverOutcome === undefined) return null;

  return (
    <div className="fixed inset-x-0 bottom-0 z-50 max-h-[60vh] overflow-y-auto rounded-t-2xl bg-white p-4 pb-6 shadow-[0_-4px_16px_rgba(0,0,0,0.15)] dark:bg-zinc-900">
      {gameOverOutcome !== undefined && <GameOverMessage outcome={gameOverOutcome} />}

      {drawChoice && (
        <div className="flex flex-col gap-2">
          <h2 className="text-sm font-semibold">Draw a card</h2>
          <button
            type="button"
            onClick={() => drawChoice.onSubmit(null)}
            className="w-full rounded bg-foreground px-3 py-2 text-left text-sm text-background"
          >
            Draw normally (top of deck)
          </button>
          {drawChoice.candidates.length > 0 && (
            <>
              <p className="text-xs text-zinc-500">Or choose a specific card:</p>
              {drawChoice.candidates.map((card, i) => (
                <button
                  key={i}
                  type="button"
                  onClick={() => drawChoice.onSubmit(card)}
                  className="w-full rounded border border-zinc-300 px-3 py-2 text-left text-sm dark:border-zinc-700"
                >
                  {cardName(card)}
                </button>
              ))}
            </>
          )}
        </div>
      )}

      {unmapped.length > 0 && (
        <div className="flex flex-col gap-2">
          <h2 className="text-sm font-semibold">Other actions</h2>
          {unmapped.map((action, i) => (
            <button
              key={i}
              type="button"
              onClick={() => onSubmitAction(action)}
              className="w-full rounded border border-zinc-300 px-3 py-2 text-left text-sm dark:border-zinc-700"
            >
              {describeAction(action, state)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function GameOverMessage({ outcome }: { outcome: GameOutcome | null }) {
  if (!outcome) return <p className="text-sm">The game ended with no winner.</p>;
  if (outcome === "Tie") return <p className="text-sm font-semibold">It&apos;s a tie!</p>;
  return <p className="text-sm font-semibold">Player {outcome.Win + 1} wins!</p>;
}
