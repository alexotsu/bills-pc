"use client";

import { useEffect } from "react";

/**
 * Binds digit keys 1-9 to the correspondingly-numbered item in `items` — matching the visible
 * "1. 2. 3. ..." numbering next to each button in ActionPanel's lists — so working through a
 * string of one-choice decisions doesn't require a mouse click every single time. Only binds
 * within the current page's worth of items (at most 9), consistent with what's actually visible
 * and numbered on screen.
 */
export function useNumberKeySelection<T>(items: T[], onSelect: (item: T) => void) {
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const target = e.target as HTMLElement | null;
      if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
      const index = Number(e.key) - 1;
      if (!Number.isInteger(index) || index < 0 || index >= items.length) return;
      onSelect(items[index]);
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [items, onSelect]);
}
