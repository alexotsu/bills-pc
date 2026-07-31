"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import ActionPanel from "@/components/board/ActionPanel";
import BoardLayout from "@/components/board/BoardLayout";
import DeclareWinnerModal from "@/components/board/DeclareWinnerModal";
import MobileBoardLayout from "@/components/board/mobile/MobileBoardLayout";
import { useIsMobile } from "@/hooks/useIsMobile";
import { RANDOM_STARTING_PLAYER, useWasmGame } from "@/hooks/useWasmGame";
import { ApiRequestError, fetchDecks, type Deck } from "@/lib/api";

type StartedGame = {
  deckAId: string;
  deckBId: string;
  deckAText: string;
  deckBText: string;
  seed: bigint;
  overrideDraws: boolean;
  startingPlayer: number;
  autoAdvanceForcedActions: boolean;
};

export default function PlayPage() {
  const [decks, setDecks] = useState<Deck[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [deckAId, setDeckAId] = useState("");
  const [deckBId, setDeckBId] = useState("");
  // Off by default: most players want plain random draws, and reserve manually picking cards
  // for the rarer case they're deliberately simulating a specific opening or top-deck.
  const [overrideDraws, setOverrideDraws] = useState(false);
  const [startingPlayer, setStartingPlayer] = useState(RANDOM_STARTING_PLAYER);
  // Off by default too: silently auto-ending a turn (or auto-playing a forced single-Basic
  // opening hand) with no confirmation can be jarring if you're not expecting it.
  const [autoAdvanceForcedActions, setAutoAdvanceForcedActions] = useState(false);
  const [started, setStarted] = useState<StartedGame | null>(null);

  useEffect(() => {
    fetchDecks()
      .then((fetched) => {
        setDecks(fetched);
        if (fetched.length > 0) setDeckAId(fetched[0].id);
        if (fetched.length > 1) setDeckBId(fetched[1].id);
      })
      .catch((err) => setError(err instanceof ApiRequestError ? err.message : String(err)));
  }, []);

  if (started) {
    return (
      <GameBoard
        deckAId={started.deckAId}
        deckBId={started.deckBId}
        deckAText={started.deckAText}
        deckBText={started.deckBText}
        seed={started.seed}
        overrideDraws={started.overrideDraws}
        startingPlayer={started.startingPlayer}
        autoAdvanceForcedActions={started.autoAdvanceForcedActions}
        onRestart={() => setStarted(null)}
      />
    );
  }

  if (error) {
    return (
      <main className="mx-auto max-w-md px-6 py-16">
        <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
      </main>
    );
  }
  if (!decks) {
    return (
      <main className="mx-auto max-w-md px-6 py-16">
        <p className="text-sm text-zinc-600 dark:text-zinc-400">Loading decks...</p>
      </main>
    );
  }
  if (decks.length < 2) {
    return (
      <main className="mx-auto max-w-md px-6 py-16">
        <p className="text-sm text-zinc-600 dark:text-zinc-400">
          You need at least 2 decks to play (your own decks and/or reference decks count).{" "}
          <Link href="/decks/new" className="underline">
            Build one
          </Link>
          .
        </p>
      </main>
    );
  }

  const deckA = decks.find((d) => d.id === deckAId);
  const deckB = decks.find((d) => d.id === deckBId);

  return (
    <main className="mx-auto flex max-w-md flex-col gap-6 px-6 py-16">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">Start a hotseat game</h1>
        <Link href="/games" className="text-sm underline">
          Battle history
        </Link>
      </div>
      <p className="text-sm text-zinc-600 dark:text-zinc-400">
        One player piloting both sides to simulate games faster — the side not currently acting
        has its hand hidden by default.
      </p>

      <label className="flex flex-col gap-1">
        <span className="text-sm font-medium">Player 1&apos;s deck</span>
        <select
          value={deckAId}
          onChange={(e) => setDeckAId(e.target.value)}
          className="rounded border border-zinc-300 px-3 py-2 dark:border-zinc-700 dark:bg-zinc-900"
        >
          {decks.map((d) => (
            <option key={d.id} value={d.id}>
              {d.name}
              {d.is_reference ? " (reference)" : ""}
            </option>
          ))}
        </select>
      </label>

      <label className="flex flex-col gap-1">
        <span className="text-sm font-medium">Player 2&apos;s deck</span>
        <select
          value={deckBId}
          onChange={(e) => setDeckBId(e.target.value)}
          className="rounded border border-zinc-300 px-3 py-2 dark:border-zinc-700 dark:bg-zinc-900"
        >
          {decks.map((d) => (
            <option key={d.id} value={d.id}>
              {d.name}
              {d.is_reference ? " (reference)" : ""}
            </option>
          ))}
        </select>
      </label>

      <label className="flex flex-col gap-1">
        <span className="text-sm font-medium">Who goes first?</span>
        <select
          value={startingPlayer}
          onChange={(e) => setStartingPlayer(Number(e.target.value))}
          className="rounded border border-zinc-300 px-3 py-2 dark:border-zinc-700 dark:bg-zinc-900"
        >
          <option value={RANDOM_STARTING_PLAYER}>Random (coin flip)</option>
          <option value={0}>Player 1</option>
          <option value={1}>Player 2</option>
        </select>
      </label>

      <label className="flex items-start gap-2 text-sm">
        <input
          type="checkbox"
          checked={overrideDraws}
          onChange={(e) => setOverrideDraws(e.target.checked)}
          className="mt-1"
        />
        <span>
          Let me pick my own draws instead of random (for deliberately simulating a specific
          opening hand or top-deck)
        </span>
      </label>

      <label className="flex items-start gap-2 text-sm">
        <input
          type="checkbox"
          checked={autoAdvanceForcedActions}
          onChange={(e) => setAutoAdvanceForcedActions(e.target.checked)}
          className="mt-1"
        />
        <span>
          Automatically end my turn (and auto-play a forced opening hand) whenever there&apos;s
          no other option, instead of pausing to confirm
        </span>
      </label>

      <button
        type="button"
        disabled={!deckA || !deckB}
        onClick={() => {
          if (!deckA || !deckB) return;
          setStarted({
            deckAId: deckA.id,
            deckBId: deckB.id,
            deckAText: deckA.deck_text,
            deckBText: deckB.deck_text,
            seed: BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER)),
            overrideDraws,
            startingPlayer,
            autoAdvanceForcedActions,
          });
        }}
        className="rounded bg-foreground px-4 py-2 text-background disabled:opacity-50"
      >
        Start Game
      </button>
    </main>
  );
}

function GameBoard({
  deckAId,
  deckBId,
  deckAText,
  deckBText,
  seed,
  overrideDraws,
  startingPlayer,
  autoAdvanceForcedActions,
  onRestart,
}: {
  deckAId: string;
  deckBId: string;
  deckAText: string;
  deckBText: string;
  seed: bigint;
  overrideDraws: boolean;
  startingPlayer: number;
  autoAdvanceForcedActions: boolean;
  onRestart: () => void;
}) {
  const {
    loading,
    error,
    pending,
    state,
    canUndo,
    gameId,
    submitAction,
    submitActionThenChoose,
    submitDraw,
    undo,
    declareWinner,
  } = useWasmGame(
    deckAId,
    deckBId,
    deckAText,
    deckBText,
    seed,
    overrideDraws,
    startingPlayer,
    autoAdvanceForcedActions,
  );
  const [showDeclareWinner, setShowDeclareWinner] = useState(false);
  const isMobile = useIsMobile();

  if (loading) {
    return <p className="p-8 text-sm text-zinc-600 dark:text-zinc-400">Loading engine...</p>;
  }
  if (error) {
    return <p className="p-8 text-sm text-red-600 dark:text-red-400">{error}</p>;
  }
  if (!pending || !state) {
    return null;
  }

  if (isMobile) {
    return (
      <MobileBoardLayout
        pending={pending}
        state={state}
        canUndo={canUndo}
        gameId={gameId}
        submitAction={submitAction}
        submitActionThenChoose={submitActionThenChoose}
        submitDraw={submitDraw}
        undo={undo}
        declareWinner={declareWinner}
        onRestart={onRestart}
      />
    );
  }

  // Hand visibility follows whoever's currently acting directly — no separate "pass the
  // device" confirmation step. This tool is for one person piloting both sides to simulate
  // games faster, not two people physically handing off a device, so gating on an extra click
  // per turn would just be friction with no one to protect the hidden hand from.
  const bottomSeat = pending.kind === "game_over" ? state.current_player : pending.actor;
  const handRevealed: [boolean, boolean] =
    pending.kind === "game_over" ? [true, true] : [pending.actor === 0, pending.actor === 1];

  return (
    <main className="mx-auto flex max-w-4xl flex-col gap-4 px-4 py-8">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Hotseat Game</h1>
        <div className="flex items-center gap-2">
          {pending.kind === "game_over" && gameId && (
            <Link href={`/games/${gameId}`} className="text-sm underline">
              View saved game
            </Link>
          )}
          <button
            type="button"
            onClick={() => setShowDeclareWinner(true)}
            disabled={pending.kind === "game_over"}
            className="rounded border border-zinc-300 px-3 py-1.5 text-sm disabled:opacity-30 dark:border-zinc-700"
          >
            Declare Winner
          </button>
          <button
            type="button"
            onClick={onRestart}
            className="rounded border border-zinc-300 px-3 py-1.5 text-sm dark:border-zinc-700"
          >
            New Game
          </button>
        </div>
      </div>
      <BoardLayout state={state} bottomSeat={bottomSeat} handRevealed={handRevealed} />
      <ActionPanel
        pending={pending}
        state={state}
        onSubmitAction={submitAction}
        onSubmitDraw={submitDraw}
        canUndo={canUndo}
        onUndo={undo}
      />
      {showDeclareWinner && (
        <DeclareWinnerModal
          onDeclare={(outcome) => {
            declareWinner(outcome);
            setShowDeclareWinner(false);
          }}
          onClose={() => setShowDeclareWinner(false)}
        />
      )}
    </main>
  );
}

