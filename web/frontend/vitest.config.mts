import path from "node:path";
import { defineConfig } from "vitest/config";

// Minimal config, scoped to the one thing under test right now: gestureActions.ts's pure
// matching logic. No jsdom/React Testing Library setup — nothing here touches the DOM.
export default defineConfig({
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
});
