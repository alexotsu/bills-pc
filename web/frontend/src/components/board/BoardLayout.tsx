import type { State } from "@/lib/gameTypes";
import DiscardPile from "./DiscardPile";
import EnergyZone from "./EnergyZone";
import HandRow from "./HandRow";
import PokemonSlot from "./PokemonSlot";
import StadiumSlot from "./StadiumSlot";

function PlayerRow({
  seat,
  state,
  handRevealed,
}: {
  seat: number;
  state: State;
  handRevealed: boolean;
}) {
  const board = state.in_play_pokemon[seat];
  return (
    <div className="flex flex-col gap-2 rounded border border-zinc-200 p-3 dark:border-zinc-800">
      <div className="flex items-center justify-between text-xs text-zinc-500">
        <span>
          Player {seat + 1}
          {state.current_player === seat && !state.winner && " — current turn"}
        </span>
        <span>Points: {state.points[seat]}</span>
      </div>
      <div className="flex items-end gap-3 overflow-x-auto pb-1">
        <PokemonSlot played={board[0]} label="Active" />
        <div className="flex shrink-0 gap-2">
          {[1, 2, 3].map((i) => (
            <PokemonSlot key={i} played={board[i]} label={`Bench ${i}`} />
          ))}
        </div>
        <DiscardPile cards={state.discard_piles[seat]} />
        <EnergyZone zone={state.energy_zone[seat]} />
      </div>
      <HandRow cards={state.hands[seat]} revealed={handRevealed} />
    </div>
  );
}

/**
 * Renders both players' boards plus the shared stadium slot. `bottomSeat`/`handRevealed` mirror
 * the TUI's own `App::bottom_seat`/`App::is_hand_revealed` (`src/tui/app.rs`): in hotseat mode
 * the seat currently acting renders at the bottom with its hand revealed, the other seat's hand
 * stays hidden — a UX guardrail so piloting both sides doesn't mean seeing both hands at once,
 * not a security boundary (see web/SPEC.md).
 */
export default function BoardLayout({
  state,
  bottomSeat,
  handRevealed,
}: {
  state: State;
  bottomSeat: number;
  handRevealed: [boolean, boolean];
}) {
  const topSeat = 1 - bottomSeat;
  return (
    <div className="flex flex-col gap-3">
      {/* turn_count is 0 during the opening-hand/active-placement setup phase, then increments
          once per PLAYER turn (not per round) — turn 1 is the starting player's first turn,
          turn 2 the other player's first turn, and so on (src/state/mod.rs's advance_turn). */}
      <p className="text-center text-xs font-medium text-zinc-500">
        {state.turn_count === 0 ? "Setup" : `Turn ${state.turn_count}`}
      </p>
      <PlayerRow seat={topSeat} state={state} handRevealed={handRevealed[topSeat]} />
      <div className="flex justify-center">
        <StadiumSlot
          card={state.active_stadium}
          ownerLabel={
            // Loose `!= null` deliberately: serde_wasm_bindgen serializes Rust's `None` as JS
            // `undefined`, not `null` — despite the `| null` in this field's TS type — so a
            // strict `!== null` check here would treat "no owner" as if one were always
            // present, rendering "Player NaN".
            state.active_stadium_owner != null
              ? `Player ${state.active_stadium_owner + 1}`
              : undefined
          }
        />
      </div>
      <PlayerRow seat={bottomSeat} state={state} handRevealed={handRevealed[bottomSeat]} />
    </div>
  );
}
