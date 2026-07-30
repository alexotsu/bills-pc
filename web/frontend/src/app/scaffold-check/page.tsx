"use client";

import { useEffect, useState } from "react";

// Small, real decks in the existing DeckGym text format (Deck::from_string) — same format used
// by the CLI and deckgym.com's builder ("Copy as Text"). This page exists only to prove the
// wasm engine and API backend are wired up correctly; it is not the real game UI.
const DECK_A = `Pokémon: 10
2 Bulbasaur A1 001
2 Exeggcute A1 021
2 Exeggutor ex A1 023
2 Ivysaur A1 002
2 Venusaur ex A1 004

Trainer: 10
2 Professor's Research P-A 007
2 Poké Ball P-A 005
2 Erika A1 219
1 Sabrina A1 225
2 X Speed P-A 002
1 Red Card P-A 006
`;

const DECK_B = `Pokémon: 8
2 Ekans A1 164
2 Arbok A1 165
2 Koffing A1 176
2 Weezing A1 177

Trainer: 12
2 Professor's Research P-A 007
2 Koga A1 222
2 Poké Ball P-A 005
2 Sabrina A1 225
2 Potion P-A 001
1 X Speed P-A 002
1 Giovanni A1 223
`;

export default function ScaffoldCheckPage() {
  const [wasmStatus, setWasmStatus] = useState("loading wasm module...");
  const [apiStatus, setApiStatus] = useState("checking API...");

  useEffect(() => {
    (async () => {
      try {
        // Loaded as a runtime URL (wasm-pack's "web" target), not a bundler-resolved static
        // import — sidesteps webpack-vs-Turbopack differences in wasm-loading support. TS
        // can't resolve this as a project module, hence the ts-expect-error. Both directive
        // comments must sit immediately above the target line — an intervening comment line
        // (even another directive) breaks "next-line" targeting.
        // @ts-expect-error - runtime URL import, not a project module
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const wasm: any = await import(/* webpackIgnore: true */ "/wasm-pkg/engine_wasm.js");
        await wasm.default();
        const game = new wasm.WasmGame(DECK_A, DECK_B, BigInt(42));
        const decision = game.step();
        setWasmStatus(
          `wasm engine loaded OK. First pending decision:\n${JSON.stringify(decision, null, 2)}`,
        );
      } catch (err) {
        setWasmStatus(`wasm engine load FAILED: ${String(err)}`);
      }
    })();
  }, []);

  useEffect(() => {
    const apiUrl = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
    fetch(`${apiUrl}/health`)
      .then((res) => res.json())
      .then((data) => setApiStatus(`API reachable: ${JSON.stringify(data)}`))
      .catch((err) => setApiStatus(`API unreachable: ${String(err)}`));
  }, []);

  return (
    <main style={{ padding: 24, fontFamily: "monospace" }}>
      <h1>Scaffolding check</h1>
      <p>
        Proves the wasm engine and Axum API are wired up correctly. Not the
        real game UI — see web/SPEC.md.
      </p>
      <h2>WASM engine (engine-wasm)</h2>
      <pre>{wasmStatus}</pre>
      <h2>API backend (web/api)</h2>
      <pre>{apiStatus}</pre>
    </main>
  );
}
