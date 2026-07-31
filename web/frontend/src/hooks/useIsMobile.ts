"use client";

import { useSyncExternalStore } from "react";

const MOBILE_QUERY = "(max-width: 767px)";

function subscribe(callback: () => void): () => void {
  const mql = window.matchMedia(MOBILE_QUERY);
  mql.addEventListener("change", callback);
  return () => mql.removeEventListener("change", callback);
}

function getSnapshot(): boolean {
  return window.matchMedia(MOBILE_QUERY).matches;
}

function getServerSnapshot(): boolean {
  // Assume desktop for SSR and the client's first render, so they match exactly — no hydration
  // mismatch. A real mobile device briefly renders one desktop-shaped frame before
  // useSyncExternalStore re-subscribes and reports the real value; deliberate trade-off (see the
  // mobile-board plan) rather than reaching for user-agent sniffing or a server/client split.
  return false;
}

/**
 * 767px matches Tailwind's own `md` breakpoint, already used elsewhere in this codebase.
 * `useSyncExternalStore` (not a manual `useEffect` + `setState`) is the correct tool here — this
 * is exactly "subscribe to a live value from an external browser API," which is what it exists
 * for, and it avoids the effect-triggered-cascading-render pattern a hand-rolled version would
 * need to work around.
 */
export function useIsMobile(): boolean {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}
