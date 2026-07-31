"use client";

import { useEffect, useMemo, useState } from "react";
import { ApiRequestError, fetchCards, type CardCatalogEntry } from "@/lib/api";
import {
  buildDeckText,
  checkDeckValidity,
  countsByName,
  normalizeForSearch,
  parseDeckText,
  type CardCounts,
} from "@/lib/deckBuilder";

type DeckBuilderProps = {
  initialName?: string;
  initialDeckText?: string;
  submitLabel: string;
  onSubmit: (params: { name: string; deck_text: string }) => Promise<void>;
};

export default function DeckBuilder({
  initialName = "",
  initialDeckText,
  submitLabel,
  onSubmit,
}: DeckBuilderProps) {
  const [catalog, setCatalog] = useState<CardCatalogEntry[] | null>(null);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [name, setName] = useState(initialName);
  const [counts, setCounts] = useState<CardCounts>(() =>
    initialDeckText ? parseDeckText(initialDeckText) : {},
  );
  const [search, setSearch] = useState("");
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    fetchCards()
      .then((cards) => setCatalog(cards.filter((c) => c.status === "Complete")))
      .catch((err) => setCatalogError(String(err)));
  }, []);

  // Sorted by name first (then id) so a name's different printings land next to each other in
  // the list — helpful now that they're not collapsed into one row (see countsByName below for
  // why: different printings of the same name can have different movesets, so each needs to
  // stay individually pickable).
  const sortedCatalog = useMemo(() => {
    if (!catalog) return null;
    return [...catalog].sort((a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id));
  }, [catalog]);

  const validity = useMemo(
    () => (catalog ? checkDeckValidity(counts, catalog) : null),
    [counts, catalog],
  );

  const nameTotals = useMemo(
    () => (catalog ? countsByName(counts, catalog) : {}),
    [counts, catalog],
  );

  const filteredCatalog = useMemo(() => {
    if (!sortedCatalog) return [];
    const query = normalizeForSearch(search.trim());
    if (!query) return sortedCatalog;
    return sortedCatalog.filter((c) => normalizeForSearch(c.name).includes(query));
  }, [sortedCatalog, search]);

  const selected = useMemo(() => {
    if (!sortedCatalog) return [];
    return sortedCatalog.filter((c) => (counts[c.id] ?? 0) > 0);
  }, [sortedCatalog, counts]);

  function setCount(id: string, count: number) {
    setCounts((prev) => {
      const next = { ...prev };
      if (count <= 0) {
        delete next[id];
      } else {
        next[id] = count;
      }
      return next;
    });
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!catalog || !validity?.isValid || name.trim() === "") return;
    setSubmitError(null);
    setSubmitting(true);
    try {
      await onSubmit({ name: name.trim(), deck_text: buildDeckText(counts, catalog) });
    } catch (err) {
      setSubmitError(err instanceof ApiRequestError ? err.message : "couldn't save deck");
    } finally {
      setSubmitting(false);
    }
  }

  if (catalogError) {
    return <p className="text-sm text-red-600 dark:text-red-400">{catalogError}</p>;
  }
  if (!catalog || !validity) {
    return <p className="text-sm text-zinc-600 dark:text-zinc-400">Loading cards...</p>;
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-6">
      <label className="flex flex-col gap-1">
        <span className="text-sm font-medium">Deck name</span>
        <input
          type="text"
          required
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="max-w-sm rounded border border-zinc-300 px-3 py-2 dark:border-zinc-700 dark:bg-zinc-900"
        />
      </label>

      <div className="grid grid-cols-1 gap-6 md:grid-cols-[1fr_320px]">
        <div className="flex flex-col gap-2">
          <input
            type="text"
            placeholder="Search cards..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="rounded border border-zinc-300 px-3 py-2 text-sm dark:border-zinc-700 dark:bg-zinc-900"
          />
          <ul className="flex max-h-[32rem] flex-col gap-1 overflow-y-auto">
            {filteredCatalog.map((card) => (
              <CardRow
                key={card.id}
                card={card}
                count={counts[card.id] ?? 0}
                nameTotal={nameTotals[card.name] ?? 0}
                onChange={(count) => setCount(card.id, count)}
              />
            ))}
          </ul>
        </div>

        <div className="flex flex-col gap-2 md:sticky md:top-4 md:self-start">
          <p className="text-sm font-medium">
            Your deck — {validity.totalCards} / 20 &middot; {validity.basicCount} Basic
          </p>
          {validity.errors.map((err) => (
            <p key={err} className="text-xs text-amber-600 dark:text-amber-400">
              {err}
            </p>
          ))}
          {selected.length === 0 ? (
            <p className="text-sm text-zinc-500 dark:text-zinc-500">
              Nothing selected yet — add cards from the list.
            </p>
          ) : (
            <ul className="flex max-h-[28rem] flex-col gap-1 overflow-y-auto rounded border border-zinc-200 p-2 dark:border-zinc-800">
              {selected.map((card) => (
                <CardRow
                  key={card.id}
                  card={card}
                  count={counts[card.id] ?? 0}
                  nameTotal={nameTotals[card.name] ?? 0}
                  onChange={(count) => setCount(card.id, count)}
                />
              ))}
            </ul>
          )}
        </div>
      </div>

      {submitError && <p className="text-sm text-red-600 dark:text-red-400">{submitError}</p>}

      <button
        type="submit"
        disabled={submitting || !validity.isValid || name.trim() === ""}
        className="w-fit rounded bg-foreground px-4 py-2 text-background disabled:opacity-50"
      >
        {submitting ? "Saving..." : submitLabel}
      </button>
    </form>
  );
}

function CardThumbnail({ card }: { card: Pick<CardCatalogEntry, "name" | "image_url"> }) {
  // Cards get art uploaded to the external host one at a time, so `image_url` being set doesn't
  // guarantee that particular file exists yet — falls back to the placeholder box on a 404
  // rather than showing a broken-image icon.
  const [imgFailed, setImgFailed] = useState(false);
  if (card.image_url && !imgFailed) {
    return (
      // Host isn't known at build time (see CARD_IMAGE_BASE_URL in web/api/.env.example), so
      // this can't use next/image's remotePatterns allow-list; plain <img> instead.
      // eslint-disable-next-line @next/next/no-img-element
      <img
        src={card.image_url}
        alt={card.name}
        onError={() => setImgFailed(true)}
        className="h-14 w-10 shrink-0 rounded object-cover"
      />
    );
  }
  return (
    <div className="flex h-14 w-10 shrink-0 items-center justify-center rounded border border-dashed border-zinc-300 p-0.5 text-center text-[8px] leading-tight text-zinc-400 dark:border-zinc-700">
      {card.name}
    </div>
  );
}

function CardRow({
  card,
  count,
  nameTotal,
  onChange,
}: {
  card: CardCatalogEntry;
  count: number;
  nameTotal: number;
  onChange: (count: number) => void;
}) {
  // A different printing of this same name might already be at the 2-copy cap — this disables
  // "+" for that case too, not just when *this* row's own count hits 2, since the rule is a
  // shared cap across every printing of the name (see countsByName in lib/deckBuilder.ts).
  const atNameCap = nameTotal >= 2;

  return (
    <li className="flex items-center justify-between gap-2 rounded border border-zinc-200 px-2 py-1 text-sm dark:border-zinc-800">
      <span className="flex min-w-0 items-center gap-2">
        <CardThumbnail card={card} />
        {/* Fixed width, regardless of content: without it, the "x/2 across printings" badge
            appearing only once nameTotal > 0 widens this block and shoves the +/- buttons over
            via justify-between — meaning the "+" button physically moves between your first and
            second click when going from 0 to 2 copies of a card. */}
        <span className="flex w-40 min-w-0 flex-col">
          <span className="truncate">
            {card.name} {card.is_basic && <span className="text-xs text-zinc-500">(Basic)</span>}
          </span>
          <span className="truncate text-xs text-zinc-500">
            {card.id}
            {nameTotal > 0 && ` · ${nameTotal}/2 across printings`}
          </span>
        </span>
      </span>
      <span className="flex shrink-0 items-center gap-2">
        <button
          type="button"
          onClick={() => onChange(count - 1)}
          disabled={count === 0}
          className="h-6 w-6 rounded border border-zinc-300 disabled:opacity-30 dark:border-zinc-700"
        >
          −
        </button>
        <span className="w-4 text-center">{count}</span>
        <button
          type="button"
          onClick={() => onChange(count + 1)}
          disabled={atNameCap}
          className="h-6 w-6 rounded border border-zinc-300 disabled:opacity-30 dark:border-zinc-700"
        >
          +
        </button>
      </span>
    </li>
  );
}
