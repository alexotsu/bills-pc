"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import BoardLayout from "@/components/board/BoardLayout";
import { useCurrentUser } from "@/hooks/useCurrentUser";
import { ApiRequestError, fetchGame, type GameDetail } from "@/lib/api";
import { describeAction } from "@/lib/describeAction";
import type { Action, State } from "@/lib/gameTypes";

export default function GameReplayPage() {
  const { id } = useParams<{ id: string }>();
  const { user, loading: userLoading } = useCurrentUser();
  const [game, setGame] = useState<GameDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [plyIndex, setPlyIndex] = useState(0);

  useEffect(() => {
    if (!user) return;
    fetchGame(id)
      .then(setGame)
      .catch((err) => setError(err instanceof ApiRequestError ? err.message : String(err)));
  }, [id, user]);

  if (userLoading) {
    return <main className="flex flex-1 items-center justify-center">Loading...</main>;
  }
  if (!user) {
    return (
      <main className="mx-auto max-w-md px-6 py-16">
        <p className="text-sm text-zinc-600 dark:text-zinc-400">
          <Link href="/login" className="underline">
            Log in
          </Link>{" "}
          to view this game.
        </p>
      </main>
    );
  }
  if (error) {
    return (
      <main className="mx-auto max-w-md px-6 py-16">
        <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
      </main>
    );
  }
  if (!game) {
    return (
      <main className="mx-auto max-w-md px-6 py-16">
        <p className="text-sm text-zinc-600 dark:text-zinc-400">Loading game...</p>
      </main>
    );
  }

  return (
    <main className="mx-auto flex max-w-4xl flex-col gap-4 px-4 py-8">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Replay</h1>
        <Link href="/games" className="text-sm underline">
          Back to battle history
        </Link>
      </div>

      <div className="flex flex-wrap gap-4 text-sm text-zinc-600 dark:text-zinc-400">
        <span>
          {game.deck_a_name} <span className="text-zinc-400">vs</span> {game.deck_b_name}
        </span>
        <span>Mode: {game.mode}</span>
        <span>{outcomeLabel(game)}</span>
        <span>Started: {new Date(game.created_at).toLocaleString()}</span>
      </div>

      {game.plies.length === 0 ? (
        <p className="text-sm text-zinc-600 dark:text-zinc-400">
          This game has no saved plies (likely abandoned before the first move).
        </p>
      ) : (
        <Replay plies={game.plies} plyIndex={plyIndex} onPlyIndexChange={setPlyIndex} />
      )}
    </main>
  );
}

/** Names the winning deck rather than showing a bare "Win"/"Loss" — `game.outcome` is stored
 * relative to `deck_a`/"Player 1" (see the `GameWithDeckNames` doc comment in
 * web/api/src/games.rs), an assignment that's arbitrary per game; a win for deck_a is a loss for
 * deck_b and vice versa, so the outcome belongs to a deck, not to whichever seat it started in. */
function outcomeLabel(game: GameDetail): string {
  if (game.outcome === null) return "Incomplete";
  if (game.outcome === "tie") return "Tie";
  return `${game.outcome === "win" ? game.deck_a_name : game.deck_b_name} won`;
}

function Replay({
  plies,
  plyIndex,
  onPlyIndexChange,
}: {
  plies: GameDetail["plies"];
  plyIndex: number;
  onPlyIndexChange: (index: number) => void;
}) {
  const current = plies[plyIndex];
  // Trusted data: this JSON was produced by our own wasm engine and stored as-is (see the
  // "opaque JSON" comment on GameDetail in web/api/src/games.rs) — safe to cast back to the
  // real types here rather than re-validating a shape we know is correct.
  const state = current.state as State;
  const chosenAction = current.chosen_action as Action;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-4">
        <button
          type="button"
          onClick={() => onPlyIndexChange(Math.max(0, plyIndex - 1))}
          disabled={plyIndex === 0}
          className="rounded border border-zinc-300 px-3 py-1.5 text-sm disabled:opacity-30 dark:border-zinc-700"
        >
          Prev
        </button>
        <div className="flex flex-col items-center gap-1">
          <span className="text-sm font-medium">
            Ply {plyIndex + 1} / {plies.length}
          </span>
          <input
            type="range"
            min={0}
            max={plies.length - 1}
            value={plyIndex}
            onChange={(e) => onPlyIndexChange(Number(e.target.value))}
            className="w-48"
          />
        </div>
        <button
          type="button"
          onClick={() => onPlyIndexChange(Math.min(plies.length - 1, plyIndex + 1))}
          disabled={plyIndex === plies.length - 1}
          className="rounded border border-zinc-300 px-3 py-1.5 text-sm disabled:opacity-30 dark:border-zinc-700"
        >
          Next
        </button>
      </div>

      <p className="text-center text-sm">
        Player {current.actor + 1}: {describeAction(chosenAction, state)}
      </p>

      {/* bottomSeat fixed at 0 (not following whoever's turn it is) so the board doesn't flip
          around as you step through — both hands are revealed since this is already-played
          history, nothing left to keep hidden. */}
      <BoardLayout state={state} bottomSeat={0} handRevealed={[true, true]} />
    </div>
  );
}
