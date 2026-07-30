export const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

export type User = {
  id: string;
  email: string | null;
  oauth_provider: string | null;
  training_data_opt_in: boolean;
  created_at: string;
};

export type Deck = {
  id: string;
  user_id: string | null;
  name: string;
  deck_text: string;
  is_reference: boolean;
  created_at: string;
  updated_at: string;
};

export type ImplementationStatus =
  | "Complete"
  | "CardNotFound"
  | "MissingAttack"
  | "MissingAbility"
  | "MissingTrainer"
  | "MissingTool";

export type CardCatalogEntry = {
  id: string;
  name: string;
  card_type: "pokemon" | "trainer";
  is_basic: boolean;
  status: ImplementationStatus;
  image_url: string | null;
};

export type GameOutcomeLabel = "win" | "loss" | "tie";

export type GameListItem = {
  id: string;
  deck_a_id: string;
  deck_a_name: string;
  deck_b_id: string;
  deck_b_name: string;
  mode: string;
  outcome: GameOutcomeLabel | null;
  seed: string;
  created_at: string;
  updated_at: string;
};

export type Game = {
  id: string;
  user_id: string | null;
  deck_a_id: string;
  deck_b_id: string;
  mode: string;
  outcome: GameOutcomeLabel | null;
  seed: string;
  created_at: string;
  updated_at: string;
};

/** A single saved ply, mirroring the engine's `ExportedDataPoint` (`src/data_exporter.rs`).
 * `state`/`playable_actions`/`chosen_action` are stored (and returned) as opaque JSON — see the
 * comment on `GameDetail` in `web/api/src/games.rs` for why they're untyped here too. */
export type GamePly = {
  ply: number;
  actor: number;
  state: unknown;
  playable_actions: unknown;
  chosen_action: unknown;
};

export type GameDetail = Game & { plies: GamePly[] };

export class ApiRequestError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

/**
 * `credentials: "include"` is required on every call: the API and frontend are different
 * origins in dev (localhost:8080 vs localhost:3000), and the session cookie is httpOnly/
 * SameSite=Lax, so the browser will only attach it to a cross-origin fetch that opts in
 * explicitly.
 */
async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_URL}${path}`, {
    ...init,
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
      ...init?.headers,
    },
  });

  if (!res.ok) {
    const body = await res.json().catch(() => null);
    throw new ApiRequestError(
      res.status,
      (body && typeof body === "object" && "error" in body && String(body.error)) ||
        `request to ${path} failed with ${res.status}`,
    );
  }

  if (res.status === 204) {
    return undefined as T;
  }
  return res.json() as Promise<T>;
}

export function fetchCurrentUser(): Promise<User> {
  return request<User>("/api/auth/me");
}

export function register(params: {
  email: string;
  password: string;
  training_data_opt_in: boolean;
}): Promise<User> {
  return request<User>("/api/auth/register", {
    method: "POST",
    body: JSON.stringify(params),
  });
}

export function login(params: { email: string; password: string }): Promise<User> {
  return request<User>("/api/auth/login", {
    method: "POST",
    body: JSON.stringify(params),
  });
}

export function logout(): Promise<void> {
  return request<void>("/api/auth/logout", { method: "POST" });
}

export function deleteAccount(): Promise<void> {
  return request<void>("/api/auth/account", { method: "DELETE" });
}

export function completeOAuthSignup(trainingDataOptIn: boolean): Promise<User> {
  return request<User>("/api/auth/oauth/complete", {
    method: "POST",
    body: JSON.stringify({ training_data_opt_in: trainingDataOptIn }),
  });
}

export function oauthStartUrl(provider: "google" | "facebook"): string {
  return `${API_URL}/api/auth/oauth/${provider}/start`;
}

export function fetchCards(): Promise<CardCatalogEntry[]> {
  return request<CardCatalogEntry[]>("/api/cards");
}

export function fetchDecks(): Promise<Deck[]> {
  return request<Deck[]>("/api/decks");
}

export function fetchDeck(id: string): Promise<Deck> {
  return request<Deck>(`/api/decks/${id}`);
}

export function createDeck(params: { name: string; deck_text: string }): Promise<Deck> {
  return request<Deck>("/api/decks", {
    method: "POST",
    body: JSON.stringify(params),
  });
}

export function updateDeck(
  id: string,
  params: { name: string; deck_text: string },
): Promise<Deck> {
  return request<Deck>(`/api/decks/${id}`, {
    method: "PUT",
    body: JSON.stringify(params),
  });
}

export function deleteDeck(id: string): Promise<void> {
  return request<void>(`/api/decks/${id}`, { method: "DELETE" });
}

export function createGame(params: {
  deck_a_id: string;
  deck_b_id: string;
  mode: "hotseat" | "ai";
  /** A stringified bigint — see the comment on `CreateGameRequest::seed` in
   * web/api/src/games.rs for why: JSON.stringify throws on a raw bigint. */
  seed: string;
}): Promise<Game> {
  return request<Game>("/api/games", {
    method: "POST",
    body: JSON.stringify(params),
  });
}

export function submitPlies(
  gameId: string,
  plies: {
    ply: number;
    actor: number;
    state: unknown;
    playable_actions: unknown;
    chosen_action: unknown;
  }[],
): Promise<void> {
  return request<void>(`/api/games/${gameId}/plies`, {
    method: "POST",
    body: JSON.stringify({ plies }),
  });
}

/** Removes every persisted ply at or after `from` — used when Undo reverts one or more actions,
 * so a decision the player corrected doesn't linger in the saved record. */
export function deletePliesFrom(gameId: string, from: number): Promise<void> {
  return request<void>(`/api/games/${gameId}/plies?from=${from}`, { method: "DELETE" });
}

export function updateGameOutcome(
  gameId: string,
  outcome: GameOutcomeLabel,
): Promise<Game> {
  return request<Game>(`/api/games/${gameId}`, {
    method: "PATCH",
    body: JSON.stringify({ outcome }),
  });
}

export function fetchGames(filters?: {
  outcome?: GameOutcomeLabel | "incomplete";
  deckId?: string;
}): Promise<GameListItem[]> {
  const params = new URLSearchParams();
  if (filters?.outcome) params.set("outcome", filters.outcome);
  if (filters?.deckId) params.set("deck_id", filters.deckId);
  const query = params.toString();
  return request<GameListItem[]>(`/api/games${query ? `?${query}` : ""}`);
}

export function fetchGame(id: string): Promise<GameDetail> {
  return request<GameDetail>(`/api/games/${id}`);
}
