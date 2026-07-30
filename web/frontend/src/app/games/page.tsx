"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useCurrentUser } from "@/hooks/useCurrentUser";
import {
  ApiRequestError,
  fetchDecks,
  fetchGames,
  type Deck,
  type GameListItem,
  type GameOutcomeLabel,
} from "@/lib/api";

const OUTCOME_LABELS: Record<GameOutcomeLabel | "incomplete", string> = {
  win: "Win",
  loss: "Loss",
  tie: "Tie",
  incomplete: "Incomplete",
};

export default function GamesPage() {
  const { user, loading: userLoading } = useCurrentUser();
  const [decks, setDecks] = useState<Deck[]>([]);
  const [games, setGames] = useState<GameListItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [outcomeFilter, setOutcomeFilter] = useState<GameOutcomeLabel | "incomplete" | "">("");
  const [deckFilter, setDeckFilter] = useState("");

  useEffect(() => {
    if (!user) return;
    fetchDecks()
      .then(setDecks)
      .catch(() => {
        /* deck filter is a convenience; a failed fetch here shouldn't block the games list */
      });
  }, [user]);

  useEffect(() => {
    if (!user) return;
    // Deliberately doesn't clear `games` first: the previous filter's results stay on screen
    // until the new ones arrive, rather than flashing back to "Loading..." on every change.
    fetchGames({
      outcome: outcomeFilter || undefined,
      deckId: deckFilter || undefined,
    })
      .then(setGames)
      .catch((err) => setError(err instanceof ApiRequestError ? err.message : String(err)));
  }, [user, outcomeFilter, deckFilter]);

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
          to see your battle history.
        </p>
      </main>
    );
  }

  return (
    <main className="mx-auto flex max-w-3xl flex-col gap-6 px-6 py-16">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">Battle History</h1>
        <Link href="/play" className="text-sm underline">
          Play a game
        </Link>
      </div>

      <div className="flex flex-wrap gap-4">
        <label className="flex flex-col gap-1">
          <span className="text-sm font-medium">Outcome</span>
          <select
            value={outcomeFilter}
            onChange={(e) => setOutcomeFilter(e.target.value as GameOutcomeLabel | "incomplete" | "")}
            className="rounded border border-zinc-300 px-3 py-2 text-sm dark:border-zinc-700 dark:bg-zinc-900"
          >
            <option value="">All</option>
            <option value="win">Win</option>
            <option value="loss">Loss</option>
            <option value="tie">Tie</option>
            <option value="incomplete">Incomplete</option>
          </select>
        </label>

        <label className="flex flex-col gap-1">
          <span className="text-sm font-medium">Deck</span>
          <select
            value={deckFilter}
            onChange={(e) => setDeckFilter(e.target.value)}
            className="rounded border border-zinc-300 px-3 py-2 text-sm dark:border-zinc-700 dark:bg-zinc-900"
          >
            <option value="">All decks</option>
            {decks.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
              </option>
            ))}
          </select>
        </label>
      </div>

      {error && <p className="text-sm text-red-600 dark:text-red-400">{error}</p>}
      {!games && !error && (
        <p className="text-sm text-zinc-600 dark:text-zinc-400">Loading games...</p>
      )}
      {games && games.length === 0 && (
        <p className="text-sm text-zinc-600 dark:text-zinc-400">
          No games match these filters yet.
        </p>
      )}
      {games && games.length > 0 && (
        <ul className="flex flex-col gap-2">
          {games.map((game) => (
            <li key={game.id}>
              <Link
                href={`/games/${game.id}`}
                className="flex items-center justify-between rounded border border-zinc-200 px-4 py-3 text-sm hover:bg-zinc-50 dark:border-zinc-800 dark:hover:bg-zinc-900"
              >
                <span>
                  {game.deck_a_name} <span className="text-zinc-400">vs</span> {game.deck_b_name}
                </span>
                <span className="flex items-center gap-3 text-zinc-500">
                  <OutcomeBadge outcome={game.outcome} />
                  <span>{new Date(game.updated_at).toLocaleString()}</span>
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}

function OutcomeBadge({ outcome }: { outcome: GameOutcomeLabel | null }) {
  const label = OUTCOME_LABELS[outcome ?? "incomplete"];
  const color =
    outcome === "win"
      ? "text-green-600 dark:text-green-400"
      : outcome === "loss"
        ? "text-red-600 dark:text-red-400"
        : "text-zinc-500";
  return <span className={`text-xs font-medium ${color}`}>{label}</span>;
}
