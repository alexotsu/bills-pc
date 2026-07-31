"use client";

/** Small tappable indicator overlaid on a board element with a legal one-tap action attached to
 * it — submits directly on tap, no sheet (any follow-up choice arrives as the *next* `pending`).
 * Used for both `UseAbility` (on a Pokémon slot) and `UseStadium` (on the stadium zone). */
export default function AbilityBadge({
  onTap,
  label = "Use ability",
}: {
  onTap: () => void;
  label?: string;
}) {
  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        onTap();
      }}
      aria-label={label}
      className="absolute -right-1 -top-1 flex h-6 w-6 items-center justify-center rounded-full border-2 border-white bg-purple-600 text-[11px] text-white shadow dark:border-zinc-900"
    >
      ✦
    </button>
  );
}
