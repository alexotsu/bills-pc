"use client";

import { useRouter } from "next/navigation";
import DeckBuilder from "@/components/DeckBuilder";
import { createDeck } from "@/lib/api";

export default function NewDeckPage() {
  const router = useRouter();

  return (
    <main className="mx-auto flex max-w-3xl flex-col gap-6 px-6 py-16">
      <h1 className="text-2xl font-semibold">New deck</h1>
      <DeckBuilder
        submitLabel="Create deck"
        onSubmit={async (params) => {
          const deck = await createDeck(params);
          router.push(`/decks/${deck.id}/edit`);
        }}
      />
    </main>
  );
}
