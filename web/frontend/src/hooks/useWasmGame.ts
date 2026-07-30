"use client";

import { useEffect, useRef, useState } from "react";
import {
  createGame,
  deletePliesFrom,
  submitPlies,
  updateGameOutcome,
  type GameOutcomeLabel,
} from "@/lib/api";
import type { Action, Card, GameOutcome, PendingDecision, State } from "@/lib/gameTypes";

type WasmGameHandle = {
  step(): PendingDecision;
  submit_action(action: Action): PendingDecision;
  submit_draw(card: Card | null | undefined): PendingDecision;
  undo(): PendingDecision;
  can_undo(): boolean;
  declare_winner(outcome: GameOutcome): PendingDecision;
  get_state(): State;
};

/** `starting_player` convention on the Rust side: 0/1 forces that seat to go first, anything
 * else (this) leaves it to the engine's own seed-driven coin flip. */
export const RANDOM_STARTING_PLAYER = -1;

export type UseWasmGameResult = {
  loading: boolean;
  error: string | null;
  pending: PendingDecision | null;
  state: State | null;
  canUndo: boolean;
  /** Set once the game row has been created server-side; null until then (or forever, if
   * persistence failed — gameplay itself never blocks on this). */
  gameId: string | null;
  submitAction: (action: Action) => void;
  submitDraw: (card: Card | null) => void;
  undo: () => void;
  declareWinner: (outcome: GameOutcome) => void;
};

/** `Win(0)` = seat 0 (deck A / "Player 1") won, matching how `web/api`'s `games.outcome` is
 * recorded relative to deck_a — see the doc comment on `GameRow` in web/api/src/models.rs. */
function outcomeToLabel(outcome: GameOutcome): GameOutcomeLabel {
  return outcome === "Tie" ? "tie" : outcome.Win === 0 ? "win" : "loss";
}

type PlyRecord = {
  ply: number;
  actor: number;
  state: State;
  playable_actions: Action[];
  chosen_action: Action;
};

/**
 * Constructs one `WasmGame` per (deckAText, deckBText, seed) and drives it through the
 * interactive control plane. After every `step`/`submit_*`/`undo` call, both `pending` and
 * `state` are refetched in full via `get_state()` — no partial/diffed updates — matching the
 * TUI's own "always refresh full state" approach (`src/tui/app.rs`), which keeps this hook
 * simple and immune to ever getting out of sync with the actual engine state.
 *
 * Also persists the game: creates a `games` row on mount (`deckAId`/`deckBId`), captures a ply
 * (mirroring the engine's own `ExportedDataPoint`) on every `submit_action`/`submit_draw`, and
 * PATCHes the outcome the moment `game_over` is reached (naturally or via `declareWinner`).
 * Plies sync to the backend immediately after each one (not batched on a timer, and NOT via
 * `navigator.sendBeacon` on tab-close despite that being the more obvious choice): the session
 * cookie is `SameSite=Lax`, and Lax cookies aren't sent on cross-origin POSTs that aren't a
 * top-level navigation — which is exactly what both a beacon and a `fetch(..., {keepalive:
 * true})` flush would be here (frontend and API are different origins). Syncing after every ply
 * sidesteps that entirely: nothing is ever buffered long enough for a beacon to matter, given a
 * hotseat game's human-paced ply rate. A short interval retries anything still unsynced (e.g.
 * after a transient network failure) as a backstop.
 */
export function useWasmGame(
  deckAId: string,
  deckBId: string,
  deckAText: string,
  deckBText: string,
  seed: bigint,
  overrideDraws: boolean,
  startingPlayer: number = RANDOM_STARTING_PLAYER,
  autoAdvanceForcedActions: boolean = false,
): UseWasmGameResult {
  const gameRef = useRef<WasmGameHandle | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<PendingDecision | null>(null);
  const [state, setState] = useState<State | null>(null);
  const [canUndo, setCanUndo] = useState(false);
  const [gameId, setGameId] = useState<string | null>(null);

  const gameIdRef = useRef<string | null>(null);
  const plyCounterRef = useRef(0);
  const pliesRef = useRef<PlyRecord[]>([]);
  const syncedCountRef = useRef(0);
  const outcomeSyncedRef = useRef(false);

  async function syncPlies() {
    const id = gameIdRef.current;
    if (!id) return;
    const unsynced = pliesRef.current.slice(syncedCountRef.current);
    if (unsynced.length === 0) return;
    try {
      await submitPlies(id, unsynced);
      syncedCountRef.current = pliesRef.current.length;
    } catch (err) {
      console.error("failed to sync plies (will retry)", err);
    }
  }

  async function syncOutcomeIfGameOver(decision: PendingDecision) {
    const id = gameIdRef.current;
    if (!id || decision.kind !== "game_over" || !decision.outcome || outcomeSyncedRef.current) {
      return;
    }
    outcomeSyncedRef.current = true;
    try {
      await updateGameOutcome(id, outcomeToLabel(decision.outcome));
    } catch (err) {
      console.error("failed to sync game outcome", err);
      outcomeSyncedRef.current = false; // allow a retry on the next game_over-triggering call
    }
  }

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        // Loaded as a runtime URL (wasm-pack's "web" target), not a bundler-resolved static
        // import — see web/frontend/src/app/scaffold-check/page.tsx for why. Both directive
        // comments must sit immediately above the target line.
        // @ts-expect-error - runtime URL import, not a project module
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const wasm: any = await import(/* webpackIgnore: true */ "/wasm-pkg/engine_wasm.js");
        await wasm.default();
        if (cancelled) return;

        const game: WasmGameHandle = new wasm.WasmGame(
          deckAText,
          deckBText,
          seed,
          overrideDraws,
          startingPlayer,
          autoAdvanceForcedActions,
        );
        gameRef.current = game;
        const decision = game.step();
        if (cancelled) return;
        setPending(decision);
        setState(game.get_state());
        setCanUndo(game.can_undo());

        // Persistence is layered on top, not a gameplay dependency: fire-and-forget so a slow
        // or failed create doesn't block or crash the game itself.
        createGame({ deck_a_id: deckAId, deck_b_id: deckBId, mode: "hotseat", seed: seed.toString() })
          .then((created) => {
            if (cancelled) return;
            gameIdRef.current = created.id;
            setGameId(created.id);
            void syncPlies();
            void syncOutcomeIfGameOver(decision);
          })
          .catch((err) => console.error("failed to create persisted game row", err));
      } catch (err) {
        if (!cancelled) setError(String(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [
    deckAId,
    deckBId,
    deckAText,
    deckBText,
    seed,
    overrideDraws,
    startingPlayer,
    autoAdvanceForcedActions,
  ]);

  useEffect(() => {
    const interval = setInterval(() => void syncPlies(), 5000);
    return () => clearInterval(interval);
  }, []);

  function refreshFrom(decision: PendingDecision) {
    const game = gameRef.current;
    if (!game) return;
    setPending(decision);
    setState(game.get_state());
    setCanUndo(game.can_undo());
    void syncOutcomeIfGameOver(decision);
  }

  function capturePly(actor: number, stateBefore: State, playableActions: Action[], chosenAction: Action) {
    pliesRef.current.push({
      ply: plyCounterRef.current++,
      actor,
      state: stateBefore,
      playable_actions: playableActions,
      chosen_action: chosenAction,
    });
    void syncPlies();
  }

  function submitAction(action: Action) {
    const game = gameRef.current;
    if (!game || !pending || pending.kind !== "awaiting_action" || !state) return;
    try {
      const decision = game.submit_action(action);
      capturePly(pending.actor, state, pending.actions, action);
      refreshFrom(decision);
    } catch (err) {
      setError(String(err));
    }
  }

  function submitDraw(card: Card | null) {
    const game = gameRef.current;
    if (!game || !pending || pending.kind !== "awaiting_draw" || !state) return;
    try {
      const decision = game.submit_draw(card);
      // The engine's own SimpleAction::DrawCard doesn't carry which specific card was
      // force-drawn (submit_draw reorders the deck first, then applies the same plain
      // DrawCard action either way) — the override's effect shows up in the resulting state,
      // not the action itself, so this reconstruction is exactly what a bulk-simulated export
      // of the same ply would contain.
      const drawAction: Action = {
        actor: pending.actor,
        action: { DrawCard: { amount: pending.amount, source: pending.source } },
        is_stack: false,
      };
      capturePly(pending.actor, state, [drawAction], drawAction);
      refreshFrom(decision);
    } catch (err) {
      setError(String(err));
    }
  }

  function undo() {
    const game = gameRef.current;
    if (!game) return;
    try {
      const decision = game.undo();

      // Drop the ply the undone action would have recorded — a decision the player corrected
      // has no business staying in what's meant to be a clean training-data record. If it had
      // already synced, this also deletes it server-side (harmless no-op otherwise).
      const poppedPly = pliesRef.current.pop();
      if (poppedPly) {
        plyCounterRef.current = poppedPly.ply;
        syncedCountRef.current = Math.min(syncedCountRef.current, pliesRef.current.length);
        const id = gameIdRef.current;
        if (id) {
          deletePliesFrom(id, poppedPly.ply).catch((err) =>
            console.error("failed to delete undone ply", err),
          );
        }
      }
      // Undoing out of a just-reached game_over means it's not really over anymore — allow a
      // later, genuine conclusion to sync its outcome again.
      if (decision.kind !== "game_over") {
        outcomeSyncedRef.current = false;
      }

      refreshFrom(decision);
    } catch (err) {
      setError(String(err));
    }
  }

  function declareWinner(outcome: GameOutcome) {
    const game = gameRef.current;
    if (!game) return;
    try {
      refreshFrom(game.declare_winner(outcome));
    } catch (err) {
      setError(String(err));
    }
  }

  return {
    loading,
    error,
    pending,
    state,
    canUndo,
    gameId,
    submitAction,
    submitDraw,
    undo,
    declareWinner,
  };
}
