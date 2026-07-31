"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useCurrentUser } from "@/hooks/useCurrentUser";
import {
  ApiRequestError,
  fetchDecks,
  fetchGames,
  outcomeForDeck,
  type Deck,
  type GameListItem,
  type GameOutcomeLabel,
} from "@/lib/api";

export default function GamesPage() {
  const { user, loading: userLoading } = useCurrentUser();
  const [decks, setDecks] = useState<Deck[]>([]);
  const [games, setGames] = useState<GameListItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Defaults to hiding incomplete games — an in-progress or abandoned game isn't usually what
  // you're looking for when checking your history. "All" (value "") is still one click away.
  const [outcomeFilter, setOutcomeFilter] = useState<
    GameOutcomeLabel | "incomplete" | "completed" | ""
  >("completed");
  const [deckFilter, setDeckFilter] = useState("");
  // Only meaningful once a primary deck is also picked — narrows to the head-to-head matchup
  // between exactly these two decks (either seat order).
  const [opponentDeckFilter, setOpponentDeckFilter] = useState("");
  // The matchup record, fetched separately from `games`: it always covers every *completed*
  // game between the pair regardless of the Outcome dropdown above, so e.g. filtering the list
  // to "Win" doesn't also shrink the record's own denominator down to just the wins.
  const [headToHead, setHeadToHead] = useState<GameListItem[] | null>(null);

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
      opponentDeckId: deckFilter && opponentDeckFilter ? opponentDeckFilter : undefined,
    })
      .then(setGames)
      .catch((err) => setError(err instanceof ApiRequestError ? err.message : String(err)));
  }, [user, outcomeFilter, deckFilter, opponentDeckFilter]);

  useEffect(() => {
    // Both sides of the matchup are required — there's no single-deck "head-to-head" record.
    if (!user || !deckFilter || !opponentDeckFilter) return;
    fetchGames({
      deckId: deckFilter,
      opponentDeckId: opponentDeckFilter,
      outcome: "completed",
    })
      .then(setHeadToHead)
      .catch(() => {
        /* the record is a bonus summary on top of the list; a failed fetch shouldn't block it */
      });
  }, [user, deckFilter, opponentDeckFilter]);

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
            onChange={(e) =>
              setOutcomeFilter(
                e.target.value as GameOutcomeLabel | "incomplete" | "completed" | "",
              )
            }
            className="rounded border border-zinc-300 px-3 py-2 text-sm dark:border-zinc-700 dark:bg-zinc-900"
          >
            <option value="completed">Completed</option>
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
            onChange={(e) => {
              setDeckFilter(e.target.value);
              // An opponent filter without a primary deck is meaningless — drop it so a
              // stale selection doesn't linger invisibly if a deck is picked again later.
              if (!e.target.value) setOpponentDeckFilter("");
            }}
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

        <label className="flex flex-col gap-1">
          <span className="text-sm font-medium">Opponent deck</span>
          <select
            value={opponentDeckFilter}
            onChange={(e) => setOpponentDeckFilter(e.target.value)}
            disabled={!deckFilter}
            className="rounded border border-zinc-300 px-3 py-2 text-sm disabled:opacity-40 dark:border-zinc-700 dark:bg-zinc-900"
          >
            <option value="">Any opponent</option>
            {decks.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
              </option>
            ))}
          </select>
        </label>
      </div>

      {deckFilter && opponentDeckFilter && (
        <HeadToHeadSummary
          games={headToHead}
          deckId={deckFilter}
          deckName={decks.find((d) => d.id === deckFilter)?.name ?? "This deck"}
          opponentName={decks.find((d) => d.id === opponentDeckFilter)?.name ?? "the opponent"}
        />
      )}

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
                  <OutcomeBadge game={game} />
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

/** The head-to-head record for `deckId` against a specific opponent deck — every *completed*
 * game between exactly that pair, win/loss percentage computed relative to `deckId` (flip which
 * deck is "primary" and the same games report the opponent's own percentage instead). */
function HeadToHeadSummary({
  games,
  deckId,
  deckName,
  opponentName,
}: {
  games: GameListItem[] | null;
  deckId: string;
  deckName: string;
  opponentName: string;
}) {
  if (!games) {
    return <p className="text-sm text-zinc-600 dark:text-zinc-400">Loading matchup record...</p>;
  }
  if (games.length === 0) {
    return (
      <p className="text-sm text-zinc-600 dark:text-zinc-400">
        No completed games yet between {deckName} and {opponentName}.
      </p>
    );
  }

  const wins = games.filter((g) => outcomeForDeck(g, deckId) === "win").length;
  const losses = games.filter((g) => outcomeForDeck(g, deckId) === "loss").length;
  const ties = games.length - wins - losses;
  const winRate = Math.round((wins / games.length) * 100);

  return (
    <div className="rounded border border-zinc-200 px-4 py-3 text-sm dark:border-zinc-800">
      <span className="font-medium">{deckName}</span> has won {wins} of {games.length} games (
      {winRate}%) against <span className="font-medium">{opponentName}</span>
      {ties > 0 && ` — ${losses} loss${losses === 1 ? "" : "es"}, ${ties} tie${ties === 1 ? "" : "s"}`}
      {ties === 0 && losses > 0 && ` — ${losses} loss${losses === 1 ? "" : "es"}`}.
    </div>
  );
}

/** Names the actual winning deck rather than showing a bare "Win"/"Loss" — those are only
 * meaningful relative to *someone*, and `game.outcome` is stored relative to `deck_a`/"Player 1"
 * (see the `GameListItem` doc comment in web/api/src/games.rs), an assignment that's arbitrary
 * per game. A win for deck_a is a loss for deck_b and vice versa, so wins/losses belong to
 * decks, not to whichever seat a deck happened to start in. */
function OutcomeBadge({ game }: { game: GameListItem }) {
  if (game.outcome === null) {
    return <span className="text-xs font-medium text-zinc-500">Incomplete</span>;
  }
  if (game.outcome === "tie") {
    return <span className="text-xs font-medium text-zinc-500">Tie</span>;
  }
  const winnerName = game.outcome === "win" ? game.deck_a_name : game.deck_b_name;
  return (
    <span className="text-xs font-medium text-green-600 dark:text-green-400">
      {winnerName} won
    </span>
  );
}
