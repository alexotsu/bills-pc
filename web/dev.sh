#!/usr/bin/env bash
# One-command local dev startup: Postgres (Docker) -> migrations -> API server -> frontend.
#
# THIS FILE IS EXPECTED TO GROW: whenever a phase of web/README.md's "Running locally" section
# gains a new prerequisite or startup step (a new migration, a new required env var, a wasm
# rebuild trigger, etc.), add it here too, in the same numbered-step style, so `./dev.sh` stays
# the single source of truth and nobody has to remember a growing manual checklist.
#
# Usage: ./dev.sh [--rebuild-wasm]
#   --rebuild-wasm   Force a wasm-pack rebuild of engine-wasm even if public/wasm-pkg already
#                     has output. Do this after changing engine-wasm or the root deckgym crate.

set -euo pipefail
# Job control: makes each `... &` background job below the leader of its own new process
# group, so its descendants (cargo's spawned `api` binary, npm's spawned `next`/turbopack
# process) can be killed as a unit in cleanup() — killing just the job's own PID leaves those
# grandchildren running as orphans, which is what happened before this was added.
set -m
cd "$(dirname "${BASH_SOURCE[0]}")"

REBUILD_WASM=false
for arg in "$@"; do
  case "$arg" in
    --rebuild-wasm) REBUILD_WASM=true ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 1
      ;;
  esac
done

MIN_NODE_MAJOR=20
MIN_NODE_MINOR=9

log() { printf '\033[1;34m==>\033[0m %s\n' "$1"; }

# --- 1. Docker daemon ------------------------------------------------------------------------

log "Checking Docker is running..."
if ! docker info >/dev/null 2>&1; then
  echo "Docker daemon isn't reachable. Start Docker Desktop, then re-run this script." >&2
  exit 1
fi

# --- 2. Postgres -----------------------------------------------------------------------------

log "Starting Postgres (docker compose)..."
docker compose up -d

log "Waiting for Postgres to be healthy..."
for _ in $(seq 1 30); do
  status="$(docker compose ps --format '{{.Health}}' postgres 2>/dev/null || true)"
  if [ "$status" = "healthy" ]; then
    break
  fi
  sleep 1
done
if [ "$status" != "healthy" ]; then
  echo "Postgres didn't become healthy in time. Check 'docker compose logs postgres'." >&2
  exit 1
fi

# --- 3. API env + migrations -------------------------------------------------------------------

if [ ! -f api/.env ]; then
  log "api/.env missing, creating from api/.env.example..."
  cp api/.env.example api/.env
  echo "  (OAuth buttons won't work until you add GOOGLE_CLIENT_ID/SECRET etc. to api/.env)"
fi

if ! command -v sqlx >/dev/null 2>&1; then
  log "Installing sqlx-cli (one-time)..."
  cargo install sqlx-cli --no-default-features --features postgres,rustls
fi

log "Running database migrations..."
(cd api && sqlx migrate run)

# --- 4. Frontend env + deps ---------------------------------------------------------------------

if [ ! -f frontend/.env ]; then
  log "frontend/.env missing, creating from frontend/.env.example..."
  cp frontend/.env.example frontend/.env
fi

if [ ! -d frontend/node_modules ]; then
  log "Installing frontend dependencies (one-time)..."
  (cd frontend && npm install)
fi

# --- 5. Node version (Next.js 16 requires >= 20.9.0) --------------------------------------------

node_version_ok() {
  command -v node >/dev/null 2>&1 || return 1
  local ver major minor
  ver="$(node -v | sed 's/^v//')"
  major="${ver%%.*}"
  minor="$(echo "$ver" | cut -d. -f2)"
  [ "$major" -gt "$MIN_NODE_MAJOR" ] && return 0
  [ "$major" -eq "$MIN_NODE_MAJOR" ] && [ "$minor" -ge "$MIN_NODE_MINOR" ]
}

if ! node_version_ok; then
  log "Default Node is too old for Next.js 16 (need >=${MIN_NODE_MAJOR}.${MIN_NODE_MINOR}.0), trying nvm..."
  if [ -s "$HOME/.nvm/nvm.sh" ]; then
    # shellcheck disable=SC1091
    source "$HOME/.nvm/nvm.sh"
    nvm use --lts >/dev/null || nvm use 22 >/dev/null
  fi
  if ! node_version_ok; then
    echo "Need Node >=${MIN_NODE_MAJOR}.${MIN_NODE_MINOR}.0 on PATH (current: $(node -v 2>/dev/null || echo 'not found'))." >&2
    echo "Install one, e.g. 'nvm install 22 && nvm use 22', then re-run this script." >&2
    exit 1
  fi
fi

# --- 6. wasm-pkg -----------------------------------------------------------------------------

if [ "$REBUILD_WASM" = true ] || [ ! -f frontend/public/wasm-pkg/engine_wasm_bg.wasm ]; then
  log "Building engine-wasm..."
  if ! command -v wasm-pack >/dev/null 2>&1; then
    echo "wasm-pack not found. Install it: cargo install wasm-pack" >&2
    exit 1
  fi
  wasm-pack build engine-wasm --target web --out-dir ../frontend/public/wasm-pkg
fi

# --- 7. API server + frontend, both foregrounded so Ctrl+C stops everything ------------------

cleaned_up=false
cleanup() {
  [ "$cleaned_up" = true ] && return
  cleaned_up=true
  log "Shutting down..."
  # Negative PID = signal the whole process group, not just the wrapper subshell — needed to
  # reach cargo's/npm's actual child process (see the `set -m` comment above).
  for pid in "${api_pid:-}" "${frontend_pid:-}"; do
    [ -n "$pid" ] && kill -TERM -- "-$pid" 2>/dev/null || true
  done
}
trap cleanup EXIT INT TERM

log "Starting API server (http://localhost:8080)..."
(cd api && exec cargo run) &
api_pid=$!

log "Starting frontend (http://localhost:3000)..."
(cd frontend && exec npm run dev) &
frontend_pid=$!

log "Both starting up — logs below. Ctrl+C stops everything (Postgres keeps running in Docker)."
wait
