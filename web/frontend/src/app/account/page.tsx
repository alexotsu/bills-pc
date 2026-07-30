"use client";

import Link from "next/link";
import { useState } from "react";
import { useRouter } from "next/navigation";
import { ApiRequestError, deleteAccount } from "@/lib/api";
import { useCurrentUser } from "@/hooks/useCurrentUser";

export default function AccountPage() {
  const router = useRouter();
  const { user, loading, logout } = useCurrentUser();
  const [confirming, setConfirming] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleDelete() {
    setError(null);
    setDeleting(true);
    try {
      await deleteAccount();
      router.push("/");
    } catch (err) {
      setError(err instanceof ApiRequestError ? err.message : "couldn't delete account");
      setDeleting(false);
    }
  }

  if (loading) {
    return <main className="flex flex-1 items-center justify-center">Loading...</main>;
  }

  if (!user) {
    return (
      <main className="mx-auto flex max-w-sm flex-1 flex-col justify-center gap-4 px-6 py-16">
        <p>
          You&apos;re not logged in.{" "}
          <Link href="/login" className="font-medium underline">
            Log in
          </Link>
        </p>
      </main>
    );
  }

  return (
    <main className="mx-auto flex max-w-sm flex-1 flex-col justify-center gap-6 px-6 py-16">
      <h1 className="text-2xl font-semibold">Your account</h1>

      <dl className="flex flex-col gap-2 text-sm">
        <div className="flex justify-between">
          <dt className="text-zinc-600 dark:text-zinc-400">Email</dt>
          <dd>{user.email ?? "—"}</dd>
        </div>
        <div className="flex justify-between">
          <dt className="text-zinc-600 dark:text-zinc-400">Signed in with</dt>
          <dd>{user.oauth_provider ?? "email and password"}</dd>
        </div>
        <div className="flex justify-between">
          <dt className="text-zinc-600 dark:text-zinc-400">Training data opt-in</dt>
          <dd>{user.training_data_opt_in ? "yes" : "no"}</dd>
        </div>
      </dl>

      <button
        onClick={() => logout().then(() => router.push("/"))}
        className="rounded border border-zinc-300 px-4 py-2 text-sm dark:border-zinc-700"
      >
        Log out
      </button>

      <div className="flex flex-col gap-3 border-t border-zinc-200 pt-6 dark:border-zinc-800">
        <h2 className="text-sm font-semibold text-red-600 dark:text-red-400">Danger zone</h2>
        <p className="text-sm text-zinc-600 dark:text-zinc-400">
          Deleting your account removes your email and login info. Your past decks and games
          are kept — you already consented to their use as training data when you signed up,
          and that can&apos;t be undone by deleting the account.
        </p>

        {error && <p className="text-sm text-red-600 dark:text-red-400">{error}</p>}

        {!confirming ? (
          <button
            onClick={() => setConfirming(true)}
            className="rounded border border-red-600 px-4 py-2 text-sm text-red-600 dark:border-red-400 dark:text-red-400"
          >
            Delete account
          </button>
        ) : (
          <div className="flex flex-col gap-2">
            <p className="text-sm font-medium">Are you sure? This can&apos;t be undone.</p>
            <div className="flex gap-2">
              <button
                onClick={handleDelete}
                disabled={deleting}
                className="rounded bg-red-600 px-4 py-2 text-sm text-white disabled:opacity-50"
              >
                {deleting ? "Deleting..." : "Yes, delete my account"}
              </button>
              <button
                onClick={() => setConfirming(false)}
                disabled={deleting}
                className="rounded border border-zinc-300 px-4 py-2 text-sm dark:border-zinc-700"
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>
    </main>
  );
}
