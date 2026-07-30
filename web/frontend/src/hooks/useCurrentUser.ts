"use client";

import { useCallback, useEffect, useState } from "react";
import { ApiRequestError, fetchCurrentUser, logout as apiLogout, type User } from "@/lib/api";

export type CurrentUserState = {
  user: User | null;
  loading: boolean;
  /** Re-fetches `/api/auth/me` — call after login/register/logout/account changes. */
  refresh: () => Promise<void>;
  logout: () => Promise<void>;
};

export function useCurrentUser(): CurrentUserState {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      setUser(await fetchCurrentUser());
    } catch (err) {
      if (err instanceof ApiRequestError && err.status === 401) {
        setUser(null);
      } else {
        throw err;
      }
    }
  }, []);

  // Deliberately not just `refresh()` here: the react-hooks lint (rightly) flags an effect
  // whose body directly invokes a function it knows sets state, since that's a common source of
  // cascading-render bugs. Nesting the setState calls one level down, inside the promise
  // callbacks, is the pattern react.dev itself recommends for effect-driven data fetching (see
  // https://react.dev/learn/you-might-not-need-an-effect) and is what the lint actually checks
  // for, so the initial-load fetch is written out here instead of delegating to `refresh`.
  useEffect(() => {
    let ignore = false;
    fetchCurrentUser()
      .then((u) => {
        if (!ignore) setUser(u);
      })
      .catch((err) => {
        if (ignore) return;
        if (err instanceof ApiRequestError && err.status === 401) {
          setUser(null);
        } else {
          console.error("failed to load current user", err);
        }
      })
      .finally(() => {
        if (!ignore) setLoading(false);
      });
    return () => {
      ignore = true;
    };
  }, []);

  const logout = useCallback(async () => {
    await apiLogout();
    setUser(null);
  }, []);

  return { user, loading, refresh, logout };
}
