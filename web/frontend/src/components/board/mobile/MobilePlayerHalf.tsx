import type { State } from "@/lib/gameTypes";
import StadiumSlot from "../StadiumSlot";
import PokemonSlot from "../PokemonSlot";
import AbilityBadge from "./AbilityBadge";
import DeckPile from "./DeckPile";
import DraggableActivePokemon from "./DraggableActivePokemon";
import DroppableSlot from "./DroppableSlot";
import EnergyWidget from "./EnergyWidget";
import MobileHandRow from "./MobileHandRow";

/**
 * One player's half of the board. `mirrored` (the opponent's half, rendered at the top of the
 * screen) reflects the *entire* arrangement — not just vertically but horizontally too, matching
 * the reference screenshot: the opponent's deck sits on the opposite edge from the acting
 * player's own deck, as if the whole board were rotated 180°, not just flipped top-to-bottom.
 *
 * Only the acting player's own (non-mirrored) half is interactive — every gesture-mappable
 * action targets only the actor's own board (see `MobileBoardLayout`'s doc comment), so the
 * opponent's half only ever renders the existing (desktop) display components, no drag/drop.
 *
 * The stadium indicator sits to the side of the Active slot (not a shared middle band): since
 * `State` only ever has one `active_stadium`, rendering it on both sides but only actually
 * showing a card on whichever side owns it is what makes "whose stadium is in play" visible
 * without duplicating the same card — the *other* side's zone naturally renders as the existing
 * empty-stadium placeholder.
 *
 * `canUseStadium`/`onUseStadium` are a narrow, deliberate exception to "only the acting player's
 * own half is interactive": `SimpleAction::UseStadium` (some stadiums, e.g. Fragrant Forest,
 * have a per-turn usable effect) is always attributed to whoever's turn it is regardless of
 * which side *played* the stadium (`actor: current_player` in `src/move_generation/mod.rs`), so
 * the tap target has to live wherever the card is actually rendered — which can be the
 * opponent's mirrored half — while the action it submits is still scoped to the acting player.
 */
export default function MobilePlayerHalf({
  seat,
  state,
  mirrored,
  handRevealed,
  eligibleSlots,
  canRetreat,
  abilitySlots,
  canUseStadium,
  onTapAttack,
  onUseAbility,
  onUseStadium,
}: {
  seat: number;
  state: State;
  mirrored: boolean;
  handRevealed: boolean;
  eligibleSlots: Set<number>;
  canRetreat: boolean;
  abilitySlots: Set<number>;
  canUseStadium: boolean;
  onTapAttack: () => void;
  onUseAbility: (inPlayIdx: number) => void;
  onUseStadium: () => void;
}) {
  const board = state.in_play_pokemon[seat];
  const interactive = !mirrored;
  const showsRealStadium = state.active_stadium_owner === seat;

  const stadiumZone = (
    <div className="relative">
      <StadiumSlot
        card={showsRealStadium ? state.active_stadium : null}
        ownerLabel={showsRealStadium ? `Player ${seat + 1}` : undefined}
      />
      {showsRealStadium && canUseStadium && (
        <AbilityBadge onTap={onUseStadium} label="Use stadium" />
      )}
    </div>
  );

  const deckZone = (
    <DeckPile deckCards={state.decks[seat].cards} discardCards={state.discard_piles[seat]} />
  );

  const activeSlot = interactive ? (
    <DraggableActivePokemon
      played={board[0]}
      isValidTarget={eligibleSlots.has(0)}
      canRetreat={canRetreat}
      onTapAttack={onTapAttack}
      onUseAbility={abilitySlots.has(0) ? () => onUseAbility(0) : undefined}
    />
  ) : (
    <PokemonSlot played={board[0]} label="Active" />
  );

  const activeRow = (
    <div className="flex items-center justify-center gap-3">
      {mirrored ? deckZone : stadiumZone}
      {activeSlot}
      {mirrored ? stadiumZone : deckZone}
    </div>
  );

  const benchRow = (
    <div className="flex justify-center gap-3">
      {[1, 2, 3].map((i) =>
        interactive ? (
          <DroppableSlot
            key={i}
            index={i}
            played={board[i]}
            label={`Bench ${i}`}
            isValidTarget={eligibleSlots.has(i)}
            onUseAbility={abilitySlots.has(i) ? () => onUseAbility(i) : undefined}
          />
        ) : (
          <PokemonSlot key={i} played={board[i]} label={`Bench ${i}`} />
        ),
      )}
    </div>
  );

  const handRow = (
    <MobileHandRow cards={state.hands[seat]} revealed={handRevealed} draggable={interactive} />
  );

  const utilityRow = interactive ? (
    <div className="flex justify-end px-4">
      <EnergyWidget zone={state.energy_zone[seat]} draggable={interactive} />
    </div>
  ) : null;

  // Row order runs outward-to-innermost relative to the shared middle of the screen: the
  // opponent's hand is outermost (top edge), the acting player's hand is outermost (bottom
  // edge), and both halves' active rows sit closest to the center.
  const rows = mirrored
    ? [handRow, benchRow, activeRow]
    : [activeRow, benchRow, utilityRow, handRow];

  // Tighter than the original gap-2/py-2: a full 10-card hand can wrap to 2 rows (see
  // MobileHandRow), and this needs to leave room for that without pushing the opponent's half
  // or the shared middle band off-screen.
  return <div className="flex flex-col gap-1 py-1">{rows}</div>;
}
