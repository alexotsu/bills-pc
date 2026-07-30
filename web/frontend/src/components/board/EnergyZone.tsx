import type { EnergyZoneState } from "@/lib/gameTypes";
import EnergyDot from "./EnergyDot";

export default function EnergyZone({ zone }: { zone: EnergyZoneState }) {
  return (
    <div className="flex shrink-0 flex-col items-center gap-1 text-[10px] text-zinc-500">
      <span>Energy Zone</span>
      {zone.current ? (
        <EnergyDot type={zone.current} title={`Available: ${zone.current}`} />
      ) : (
        <span className="h-3 w-3 rounded-full border border-dashed border-zinc-400" />
      )}
      <span>Next: {zone.next ?? "—"}</span>
    </div>
  );
}
