"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import DeckBuilder from "@/components/DeckBuilder";
import { ApiRequestError, fetchDeck, updateDeck, type Deck } from "@/lib/api";

export default function EditDeckPage() {
  const { id } = useParams<{ id: string }>();
  const router = useRouter();
  const [deck, setDeck] = useState<Deck | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchDeck(id)
      .then(setDeck)
      .catch((err) => setError(err instanceof ApiRequestError ? err.message : String(err)));
  }, [id]);

  if (error) {
    return (
      <main className="mx-auto max-w-3xl px-6 py-16">
        <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
      </main>
    );
  }
  if (!deck) {
    return (
      <main className="mx-auto max-w-3xl px-6 py-16">
        <p className="text-sm text-zinc-600 dark:text-zinc-400">Loading deck...</p>
      </main>
    );
  }
  if (deck.is_reference) {
    return (
      <main className="mx-auto max-w-3xl px-6 py-16">
        <p className="text-sm text-zinc-600 dark:text-zinc-400">
          Reference decks can&apos;t be edited. Copy its cards into a new deck instead.
        </p>
      </main>
    );
  }

  return (
    <main className="mx-auto flex max-w-3xl flex-col gap-6 px-6 py-16">
      <h1 className="text-2xl font-semibold">Edit deck</h1>
      <DeckBuilder
        initialName={deck.name}
        initialDeckText={deck.deck_text}
        submitLabel="Save changes"
        onSubmit={async (params) => {
          await updateDeck(id, params);
          router.push("/decks");
        }}
      />
    </main>
  );
}
