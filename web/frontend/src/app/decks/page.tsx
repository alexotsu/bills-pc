"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { ApiRequestError, deleteDeck, fetchDecks, type Deck } from "@/lib/api";
import { useCurrentUser } from "@/hooks/useCurrentUser";

export default function DecksPage() {
  const { user, loading: userLoading } = useCurrentUser();
  const [decks, setDecks] = useState<Deck[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  useEffect(() => {
    fetchDecks()
      .then(setDecks)
      .catch((err) => setError(err instanceof ApiRequestError ? err.message : String(err)));
  }, []);

  async function handleDelete(id: string) {
    if (!confirm("Delete this deck? This can't be undone.")) return;
    setDeletingId(id);
    try {
      await deleteDeck(id);
      setDecks((prev) => prev?.filter((d) => d.id !== id) ?? null);
    } catch (err) {
      alert(err instanceof ApiRequestError ? err.message : "couldn't delete deck");
    } finally {
      setDeletingId(null);
    }
  }

  if (error) {
    return (
      <main className="mx-auto max-w-2xl px-6 py-16">
        <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
      </main>
    );
  }
  if (!decks) {
    return (
      <main className="mx-auto max-w-2xl px-6 py-16">
        <p className="text-sm text-zinc-600 dark:text-zinc-400">Loading decks...</p>
      </main>
    );
  }

  const ownDecks = decks.filter((d) => !d.is_reference);
  const referenceDecks = decks.filter((d) => d.is_reference);

  return (
    <main className="mx-auto flex max-w-2xl flex-col gap-8 px-6 py-16">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">Decks</h1>
        {!userLoading && user && (
          <Link
            href="/decks/new"
            className="rounded bg-foreground px-4 py-2 text-sm text-background"
          >
            New deck
          </Link>
        )}
      </div>

      <section className="flex flex-col gap-2">
        <h2 className="text-sm font-semibold text-zinc-600 dark:text-zinc-400">Your decks</h2>
        {!userLoading && !user && (
          <p className="text-sm text-zinc-600 dark:text-zinc-400">
            <Link href="/login" className="underline">
              Log in
            </Link>{" "}
            to build and save your own decks.
          </p>
        )}
        {user && ownDecks.length === 0 && (
          <p className="text-sm text-zinc-600 dark:text-zinc-400">
            No decks yet — start from a reference deck below or build one from scratch.
          </p>
        )}
        <DeckList decks={ownDecks} onDelete={handleDelete} deletingId={deletingId} editable />
      </section>

      <section className="flex flex-col gap-2">
        <h2 className="text-sm font-semibold text-zinc-600 dark:text-zinc-400">
          Reference decks
        </h2>
        <DeckList decks={referenceDecks} onDelete={handleDelete} deletingId={deletingId} />
      </section>
    </main>
  );
}

function DeckList({
  decks,
  onDelete,
  deletingId,
  editable = false,
}: {
  decks: Deck[];
  onDelete: (id: string) => void;
  deletingId: string | null;
  editable?: boolean;
}) {
  if (decks.length === 0) return null;
  return (
    <ul className="flex flex-col gap-2">
      {decks.map((deck) => (
        <li
          key={deck.id}
          className="flex items-center justify-between rounded border border-zinc-200 px-3 py-2 dark:border-zinc-800"
        >
          <span>{deck.name}</span>
          {editable && (
            <span className="flex gap-3 text-sm">
              <Link href={`/decks/${deck.id}/edit`} className="underline">
                Edit
              </Link>
              <button
                onClick={() => onDelete(deck.id)}
                disabled={deletingId === deck.id}
                className="text-red-600 underline disabled:opacity-50 dark:text-red-400"
              >
                Delete
              </button>
            </span>
          )}
        </li>
      ))}
    </ul>
  );
}
