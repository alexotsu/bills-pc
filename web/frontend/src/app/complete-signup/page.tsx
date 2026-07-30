"use client";

import { Suspense, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { ApiRequestError, completeOAuthSignup } from "@/lib/api";
import { useCurrentUser } from "@/hooks/useCurrentUser";

// The API redirects here after a *first-time* OAuth identity authenticates successfully, but
// before creating an account for it — see `complete_oauth_signup` in web/api/src/auth/
// handlers.rs. The verified identity is held server-side in a short-lived cookie; this page's
// only job is to collect the same explicit opt-in required on the /register form, since
// "no opt-in, no account" applies to every signup path.
function CompleteSignupForm() {
  const router = useRouter();
  const { refresh } = useCurrentUser();
  const searchParams = useSearchParams();
  const provider = searchParams.get("provider") ?? "your provider";
  const [optIn, setOptIn] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await completeOAuthSignup(optIn);
      await refresh();
      router.push("/");
    } catch (err) {
      setError(
        err instanceof ApiRequestError
          ? err.message
          : "couldn't finish signing up — please try again",
      );
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-4">
      <p className="text-sm text-zinc-600 dark:text-zinc-400">
        You signed in with {provider}. One more step to finish creating your account.
      </p>

      <label className="flex items-start gap-2 text-sm">
        <input
          type="checkbox"
          checked={optIn}
          onChange={(e) => setOptIn(e.target.checked)}
          className="mt-1"
        />
        <span>
          I agree that my gameplay data may be used to train an AI model. An account
          can&apos;t be created without this — see our data policy for details.
        </span>
      </label>

      {error && <p className="text-sm text-red-600 dark:text-red-400">{error}</p>}

      <button
        type="submit"
        disabled={submitting || !optIn}
        className="rounded bg-foreground px-4 py-2 text-background disabled:opacity-50"
      >
        {submitting ? "Finishing..." : "Finish creating account"}
      </button>
    </form>
  );
}

export default function CompleteSignupPage() {
  return (
    <main className="mx-auto flex max-w-sm flex-1 flex-col justify-center gap-6 px-6 py-16">
      <h1 className="text-2xl font-semibold">Almost done</h1>
      <Suspense fallback={null}>
        <CompleteSignupForm />
      </Suspense>
    </main>
  );
}
