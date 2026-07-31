"use client";

import { useEffect, useState } from "react";
import type { State } from "@/lib/gameTypes";

const FLASH_DURATION_MS = 200;

/**
 * Briefly flashes "Player N, Turn T" across the top of the screen whenever the turn changes.
 * Two distinct pieces: detecting the change is *derived* state (compare against a tracked
 * previous value during the render body, no effect — same pattern as `ActionPanel`'s
 * `pageResetFor`), while the 200ms auto-hide is a genuine timed side effect, which an effect is
 * the correct tool for.
 */
export default function TurnChangeFlash({ state }: { state: State }) {
  const key = `${state.current_player}:${state.turn_count}`;
  const [flashKey, setFlashKey] = useState(key);
  const [visible, setVisible] = useState(false);
  if (key !== flashKey) {
    setFlashKey(key);
    setVisible(true);
  }

  useEffect(() => {
    if (!visible) return;
    const timer = setTimeout(() => setVisible(false), FLASH_DURATION_MS);
    return () => clearTimeout(timer);
  }, [visible]);

  if (!visible) return null;

  const message =
    state.turn_count === 0 ? "Setup" : `Player ${state.current_player + 1}, Turn ${state.turn_count}`;

  return (
    <div className="pointer-events-none fixed inset-x-0 top-0 z-[60] flex justify-center pt-4">
      <div className="rounded-full bg-zinc-900/90 px-4 py-1.5 text-sm font-medium text-white shadow-lg dark:bg-white/90 dark:text-zinc-900">
        {message}
      </div>
    </div>
  );
}
