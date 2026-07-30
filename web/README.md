# deckgym web app

Scaffolding for the web interface described in [`SPEC.md`](./SPEC.md): Rust (Axum) backend,
Next.js frontend, deckgym-core compiled to WASM and run client-side for zero-latency gameplay.

**This is not a finished app yet.** Real authentication and deck CRUD are implemented; the game
board UI and battle-history persistence are not — see "What's here" below.

## Layout

```
web/
  engine-wasm/   wasm-bindgen wrapper around deckgym-core (Rust)
  api/           Axum backend: accounts, decks, battle history (Rust)
  frontend/      Next.js app (TypeScript)
  docker-compose.yml   local Postgres
```

`engine-wasm` and `api` are a Cargo workspace (`web/Cargo.toml`), independent of the root
`deckgym` package — both depend on it via a path dependency (`../..`).

## Running locally

```bash
cd web
./dev.sh
```

Starts Postgres (Docker), applies migrations, and runs the API server (`:8080`) and frontend
(`:3000`), creating `api/.env`/`frontend/.env` from the `.env.example` files on first run if
they're missing. Ctrl+C stops the API and frontend; Postgres is left running in Docker (it's a
persistent container, not something to tear down every session — `docker compose down` in `web`
if you actually want to stop it). Pass `--rebuild-wasm` to force a wasm-pack rebuild of
`engine-wasm` (needed after changing `engine-wasm` or the root `deckgym` crate; otherwise it
only builds once, the first time `public/wasm-pkg` is empty).

**This script is the source of truth for what's needed to start the app** — if you add a new
required env var, migration, or startup step, update `dev.sh`, not just this doc.

<details>
<summary>Prerequisites `dev.sh` doesn't install for you</summary>

- Rust with the `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- [`wasm-pack`](https://rustwasm.github.io/wasm-pack/): `cargo install wasm-pack` (only needed
  the first run, or with `--rebuild-wasm`)
- Docker Desktop (or another Docker daemon) — `dev.sh` needs it already running; it won't start
  it for you
- **Node.js >= 20.9.0** — Next.js 16 requires this. `dev.sh` tries `nvm` automatically if your
  default `node -v` is older; install one yourself if you don't have `nvm` (e.g. `nvm install 22`)

</details>

<details>
<summary>What `dev.sh` does, as individual manual commands (for troubleshooting)</summary>

```bash
cd web && docker compose up -d                                    # Postgres
cd web/api && cp .env.example .env && sqlx migrate run            # migrations
cd web/api && cargo run                                           # API, :8080
cd web && wasm-pack build engine-wasm --target web \
  --out-dir ../frontend/public/wasm-pkg                           # wasm engine
cd web/frontend && npm install && cp .env.example .env && npm run dev  # frontend, :3000
```

The wasm build uses the `web` target (not `bundler`) deliberately: the output is loaded via a
runtime `import()` of a URL under `public/`, not a bundler-resolved static import. This
sidesteps needing to keep up with Turbopack-vs-webpack differences in wasm-loading support — it
works the same way regardless of which one Next.js is using. The generated `public/wasm-pkg/` is
gitignored (it's build output, same as `target/`).

</details>

Visit `http://localhost:3000/register` (or `/login`) to test real auth — email/password or
Google (needs `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` in `web/api/.env`; see the OAuth section
below). Once logged in, `http://localhost:3000/decks` lists your decks and any reference decks
(viewable without logging in too), with a card-picker builder at `/decks/new` and
`/decks/:id/edit`. `http://localhost:3000/scaffold-check` still exists too — it constructs a
`WasmGame` client-side and fetches the API's `/health` endpoint; not the real game UI.

### Google/Facebook OAuth setup

To test the OAuth buttons, register an OAuth client with the provider and add the credentials
to `web/api/.env`:

- **Google** (Cloud Console → APIs & Services → Credentials → OAuth client ID, type "Web
  application"): Authorized redirect URI `http://localhost:8080/api/auth/oauth/google/callback`,
  Authorized JavaScript origin `http://localhost:3000`. Then set `GOOGLE_CLIENT_ID` /
  `GOOGLE_CLIENT_SECRET`.
- **Facebook** (developers.facebook.com, analogous setup): redirect URI
  `http://localhost:8080/api/auth/oauth/facebook/callback`. Then set `FACEBOOK_CLIENT_ID` /
  `FACEBOOK_CLIENT_SECRET`.

Restart `cargo run` after editing `.env` — env vars are only read at startup
(`Config::from_env()`). A provider whose credentials aren't set will fail only when someone
clicks that provider's button (`/api/auth/oauth/:provider/start`), not at server boot.

## What's here vs. what's not

**Here**: the wasm build pipeline, including a real `WasmGame` wrapping the engine's
interactive control plane (`step`/`submit_action`/`submit_draw`, from `Game::step`); an Axum
server connected to Postgres; the DB schema (`api/migrations/`); a Next.js app that loads the
wasm module and calls the API; real authentication — email/password (argon2-hashed) and
Google/Facebook OAuth (via the `oauth2` crate, PKCE + CSRF), DB-backed session cookies, and
GDPR-compliant account deletion (`DELETE /api/auth/account` nulls PII, keeps decks/games);
and real deck CRUD — `GET /api/cards` (the engine's full card catalog with implementation
status, via `deckgym` as a path dependency), `GET`/`POST /api/decks`, `GET`/`PUT`/`DELETE
/api/decks/:id`, validated server-side with the engine's own `Deck::from_string`/`is_valid()`
and `card_validation::get_implementation_status` (unimplemented cards can't be saved into a
deck), with a 409 if you try to edit or delete a deck that's already been used in a game.
Frontend pages at `/register`, `/login`, `/complete-signup` (the opt-in-confirmation step for
first-time OAuth signups), `/account`, `/decks`, `/decks/new`, and `/decks/:id/edit`.

**Not here yet** (separate follow-up work): the actual game board UI and battle-history
persistence logic. The `/api/games` stub route still returns placeholder data.

## Known rough edges

- `wasm-opt` is disabled in `engine-wasm/Cargo.toml`
  (`package.metadata.wasm-pack.profile.release.wasm-opt = false`) to avoid requiring the
  binaryen toolchain for this scaffolding pass. Re-enable it before shipping a real release
  build — the current unoptimized `.wasm` output is ~4MB.
