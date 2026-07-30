import type { EnergyType } from "@/lib/gameTypes";

const ENERGY_COLORS: Record<EnergyType, string> = {
  Grass: "bg-green-500",
  Fire: "bg-red-500",
  Water: "bg-blue-500",
  Lightning: "bg-yellow-400",
  Psychic: "bg-purple-500",
  Fighting: "bg-orange-700",
  Darkness: "bg-zinc-800",
  Metal: "bg-zinc-400",
  Dragon: "bg-amber-600",
  Colorless: "bg-zinc-300 dark:bg-zinc-500",
};

export default function EnergyDot({ type, title }: { type: EnergyType; title?: string }) {
  return (
    <span
      title={title ?? type}
      className={`inline-block h-3 w-3 shrink-0 rounded-full ${ENERGY_COLORS[type]}`}
    />
  );
}
