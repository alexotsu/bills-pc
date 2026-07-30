import {
  effectiveTotalHp,
  isPokemonCard,
  remainingHp,
  type PlayedCard,
} from "@/lib/gameTypes";
import EnergyDot from "./EnergyDot";

function StatusBadges({ played }: { played: PlayedCard }) {
  const badges = [
    played.poisoned && "PSN",
    played.paralyzed && "PAR",
    played.asleep && "SLP",
    played.burned && "BRN",
    played.confused && "CNF",
  ].filter(Boolean) as string[];
  if (badges.length === 0) return null;
  return (
    <div className="text-[10px] font-semibold text-red-600 dark:text-red-400">
      {badges.join(" ")}
    </div>
  );
}

export default function PokemonSlot({
  played,
  label,
}: {
  played: PlayedCard | null;
  label: string;
}) {
  if (!played) {
    return (
      <div className="flex h-40 w-28 shrink-0 flex-col items-center justify-center rounded border border-dashed border-zinc-300 text-center text-[10px] text-zinc-400 dark:border-zinc-700">
        {label}
      </div>
    );
  }

  if (!isPokemonCard(played.card)) {
    // in_play_pokemon slots only ever hold Pokémon cards — this is here purely so the type
    // system's honesty about Card being a union doesn't force a runtime crash if that ever
    // stops being true.
    return null;
  }
  const pokemon = played.card.Pokemon;

  return (
    <div className="flex w-28 shrink-0 flex-col gap-1 rounded border border-zinc-300 p-1.5 text-xs dark:border-zinc-700">
      <div className="flex items-start justify-between gap-1">
        <span className="font-semibold leading-tight">{pokemon.name}</span>
        {pokemon.weakness && (
          <EnergyDot type={pokemon.weakness} title={`Weak to ${pokemon.weakness}`} />
        )}
      </div>
      <div className="text-[11px] text-zinc-600 dark:text-zinc-400">
        {remainingHp(played)} / {effectiveTotalHp(played)} HP
      </div>
      {played.attached_tool && (
        <div className="truncate text-[10px] text-zinc-500">
          🔧 {isPokemonCard(played.attached_tool) ? played.attached_tool.Pokemon.name : played.attached_tool.Trainer.name}
        </div>
      )}
      {played.attached_energy.length > 0 && (
        <div className="flex flex-wrap gap-0.5">
          {played.attached_energy.map((energy, i) => (
            <EnergyDot key={i} type={energy} />
          ))}
        </div>
      )}
      {pokemon.attacks.length > 0 && (
        <ul className="flex flex-col gap-0.5">
          {pokemon.attacks.map((attack) => (
            <li key={attack.title} className="flex justify-between gap-1 text-[10px]">
              <span className="truncate">{attack.title}</span>
              {attack.fixed_damage > 0 && <span className="shrink-0">{attack.fixed_damage}</span>}
            </li>
          ))}
        </ul>
      )}
      <StatusBadges played={played} />
      <div className="text-[9px] text-zinc-400">
        Retreat: {pokemon.retreat_cost.length}
      </div>
    </div>
  );
}
