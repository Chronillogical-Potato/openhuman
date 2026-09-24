#!/usr/bin/env bash
# Browser-hosted variant of `pnpm dev:app`.
#
# `dev:app` renders the UI in the Tauri shell, which since the CEF -> Wry
# migration (#5456) is WKWebView on macOS / WebView2 on Windows / WebKitGTK on
# Linux. None of those speak the Chrome DevTools Protocol, so no CDP client --
# including the chrome-devtools MCP an agent drives -- can attach to the
# desktop window. This script runs the same SPA in a real browser instead:
#
#   openhuman-core (JSON-RPC :7788) <- fetch -- Vite dev server (:1420) in Chrome
#
# The renderer takes the browser path in `coreRpcClient` (`isTauri()` is false),
# reading the endpoint and bearer from `localStorage`. Those are seeded by the
# dev-server-only `/__dev-connect` page (see `devConnectPlugin` in
# `app/vite.config.ts`), which is where the browser is pointed first.
#
# Two modes:
#
# - Standalone (default): build and start a fresh `openhuman-core serve` with a
#   generated bearer. Sign-in is one click: the OAuth buttons pass
#   `<vite origin>/__dev-auth` as the backend redirectUri, and the session is
#   stored in that core's workspace.
#
# - Attach (`--attach`): reuse the core of the desktop app that is already
#   running -- and so its signed-in session. Its bearer is minted per launch and
#   held only in memory, so the browser is sent through the core's own dev-only
#   `GET /dev/connect?app=<vite origin>` (crates/openhuman-core/src/core/
#   dev_connect.rs), which redirects to `/__dev-connect` with the RPC URL and
#   bearer in the URL fragment. No token is read, printed, or pasted.
#
# Busy ports are not fatal: Vite and the standalone core each move to the next
# free port, and the URL that is printed/opened always reflects the real one.
#
# Usage:
#   pnpm dev:app:web                   # fresh core + vite, open the browser
#   pnpm dev:app:web --no-browser      # same, just print the URL (for agents)
#   pnpm dev:app:web:attach            # vite on the running desktop core
#   pnpm dev:app:web --attach --no-browser
#
# Env:
#   OPENHUMAN_DEV_PORT    preferred Vite port (default 1420)
#   OPENHUMAN_CORE_PORT   standalone: preferred core port (default 7788)
#                         attach: the desktop core's port (default: scan 7788-7808)
#   OPENHUMAN_CORE_TOKEN  standalone: bearer to use (default: generated per run)
#   OPENHUMAN_WORKSPACE   standalone: core workspace (default: ~/.openhuman)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

open_browser=1
attach=0
for arg in "$@"; do
  case "$arg" in
    --no-browser) open_browser=0 ;;
    --attach) attach=1 ;;
    *) echo "[dev:web] unknown argument: $arg" >&2; exit 2 ;;
  esac
done

if [[ -f "$REPO_ROOT/.env" ]]; then
  # shellcheck source=load-dotenv.sh
  source "$SCRIPT_DIR/load-dotenv.sh" "$REPO_ROOT/.env"
fi

port_is_free() {
  ! nc -z 127.0.0.1 "$1" >/dev/null 2>&1
}

validate_port() {
  local raw="${1//[[:space:]]/}" fallback="$2" label="$3"
  if [[ "$raw" =~ ^[0-9]+$ ]] && (( 10#$raw >= 1 && 10#$raw <= 65535 )); then
    echo "$raw"
  else
    echo "[dev:web] WARNING: invalid $label='$raw'; using $fallback" >&2
    echo "$fallback"
  fi
}

# First free port at or after $1 (within 20), else fail. Callers pass the
# result on explicitly, so a moved port is never a URL nobody is pointed at.
next_free_port() {
  local start="$1" label="$2" port="$1"
  while ! port_is_free "$port"; do
    port=$(( port + 1 ))
    if (( port > start + 20 )); then
      echo "[dev:web] ERROR: no free $label port near $start." >&2
      return 1
    fi
  done
  if [[ "$port" != "$start" ]]; then
    echo "[dev:web] port $start busy; $label will use $port" >&2
  fi
  echo "$port"
}

# HTTP status of the core's /dev/connect for a throwaway app origin: 302 means
# the route is live. The Location (which carries the bearer) is discarded.
dev_connect_status() {
  curl -s -o /dev/null -w '%{http_code}' -m 3 \
    "http://127.0.0.1:$1/dev/connect?app=http://localhost:1" 2>/dev/null || true
}

preferred_dev_port="$(validate_port "${OPENHUMAN_DEV_PORT:-1420}" 1420 OPENHUMAN_DEV_PORT)"
# Vite runs with strictPort and its HMR companion is dev port + 1, so both must
# be free; step past busy pairs.
dev_port="$(next_free_port "$preferred_dev_port" vite)"
while ! port_is_free $(( dev_port + 1 )); do
  dev_port="$(next_free_port $(( dev_port + 1 )) vite)"
done

core_pid=""
vite_pid=""
cleanup() {
  trap - EXIT INT TERM
  # Vite runs in its own process group (see below); signal the whole group so
  # the `pnpm` -> `node vite` grandchildren go too, not just the subshell.
  if [[ -n "$vite_pid" ]]; then
    kill -- "-$vite_pid" 2>/dev/null || kill "$vite_pid" 2>/dev/null || true
  fi
  [[ -n "$core_pid" ]] && kill "$core_pid" 2>/dev/null || true
  wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

if (( attach )); then
  # Find the desktop core: the one whose dev-only /dev/connect answers 302.
  # A standalone `openhuman-core serve` from this script also qualifies (it is
  # a debug build), which is fine -- it is a core with a session too.
  if [[ -n "${OPENHUMAN_CORE_PORT:-}" ]]; then
    candidates="$(validate_port "$OPENHUMAN_CORE_PORT" 7788 OPENHUMAN_CORE_PORT)"
  else
    candidates="$(seq 7788 7808)"
  fi
  core_port=""
  for port in $candidates; do
    status="$(dev_connect_status "$port")"
    if [[ "$status" == "302" ]]; then
      core_port="$port"
      break
    elif [[ "$status" == "404" ]] && curl -sf -m 2 -o /dev/null "http://127.0.0.1:$port/health"; then
      echo "[dev:web] core on :$port has no /dev/connect (release build, or older than this checkout); skipping" >&2
    fi
  done
  if [[ -z "$core_port" ]]; then
    echo "[dev:web] ERROR: no running core with /dev/connect found." >&2
    echo "[dev:web] Start the desktop app (\`pnpm dev:app\`), or set OPENHUMAN_DEV_CONNECT=1 for a" >&2
    echo "[dev:web] release build, or set OPENHUMAN_CORE_PORT to its port. Without --attach this" >&2
    echo "[dev:web] script starts its own core instead." >&2
    exit 1
  fi
  echo "[dev:web] attaching to the running core on :$core_port"
else
  preferred_core_port="$(validate_port "${OPENHUMAN_CORE_PORT:-7788}" 7788 OPENHUMAN_CORE_PORT)"
  # The core port may legitimately be taken by another checkout's (or the
  # desktop app's) core. Advance rather than reuse: standalone mode always
  # talks to the core it started itself -- use --attach to share one.
  core_port="$(next_free_port "$preferred_core_port" core)"

  # A blank OPENHUMAN_CORE_TOKEN does NOT disable auth: `init_rpc_token`
  # (crates/openhuman-core/src/core/auth.rs) trims it, treats empty as unset,
  # and falls through to generating a token and writing it to
  # {workspace}/core.token. So an explicit value is the only way both sides
  # agree on a bearer without reading that file.
  core_token="${OPENHUMAN_CORE_TOKEN:-}"
  core_token="${core_token//[[:space:]]/}"
  if [[ -z "$core_token" ]]; then
    core_token="$(openssl rand -hex 32)"
  fi
  export OPENHUMAN_CORE_TOKEN="$core_token"

  core_bin="$REPO_ROOT/target/debug/openhuman-core"
  # Always run the (incremental) build rather than only when the binary is
  # missing — the normal `tauri dev` path does the same. Skipping this once a
  # binary exists means a later run silently executes an arbitrarily stale
  # core against a current frontend, which is misleading to debug against.
  echo "[dev:web] building openhuman-core…"
  # GGML_NATIVE=OFF is the documented Apple-Silicon workaround for llama.cpp.
  GGML_NATIVE=OFF cargo build --manifest-path "$REPO_ROOT/Cargo.toml" -p openhuman-cli \
    --bin openhuman-core

  echo "[dev:web] starting openhuman-core on :$core_port"
  OPENHUMAN_CORE_PORT="$core_port" "$core_bin" serve &
  core_pid=$!

  for _ in $(seq 1 60); do
    if curl -sf -m 2 "http://127.0.0.1:$core_port/health" >/dev/null 2>&1; then
      break
    fi
    if ! kill -0 "$core_pid" 2>/dev/null; then
      echo "[dev:web] ERROR: core exited during startup." >&2
      exit 1
    fi
    sleep 1
  done

  if ! curl -sf -m 2 "http://127.0.0.1:$core_port/health" >/dev/null 2>&1; then
    echo "[dev:web] ERROR: core did not become healthy on :$core_port." >&2
    exit 1
  fi

  # Fail loudly here rather than letting the browser hit an opaque 401 later.
  rpc_status=$(curl -s -o /dev/null -w '%{http_code}' -m 10 \
    -X POST "http://127.0.0.1:$core_port/rpc" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $core_token" \
    -d '{"jsonrpc":"2.0","id":1,"method":"openhuman.auth_get_state","params":{}}')
  if [[ "$rpc_status" != "200" ]]; then
    echo "[dev:web] ERROR: authenticated RPC probe returned HTTP $rpc_status." >&2
    exit 1
  fi
  echo "[dev:web] core healthy and accepting the dev bearer"
fi

# Read by `import.meta.env` (Vite merges prefixed vars from process.env) and by
# the /__dev-connect page, which seeds it into localStorage for the browser.
# In attach mode the page takes the URL + bearer from the core's redirect.
export VITE_OPENHUMAN_CORE_RPC_URL="http://127.0.0.1:$core_port/rpc"
export OPENHUMAN_DEV_PORT="$dev_port"

echo "[dev:web] starting vite on :$dev_port"
# Job control gives the background job its own process group (pgid == pid),
# which is what lets cleanup reach the node process pnpm spawns.
set -m
(cd "$REPO_ROOT/app" && exec pnpm dev) &
vite_pid=$!
set +m

for _ in $(seq 1 60); do
  if curl -sf -m 2 -o /dev/null "http://localhost:$dev_port/"; then
    break
  fi
  if ! kill -0 "$vite_pid" 2>/dev/null; then
    echo "[dev:web] ERROR: vite exited during startup." >&2
    exit 1
  fi
  sleep 1
done

if (( attach )); then
  connect_url="http://127.0.0.1:$core_port/dev/connect?app=http://localhost:$dev_port"
else
  connect_url="http://localhost:$dev_port/__dev-connect"
fi

echo
echo "[dev:web] ready"
echo "[dev:web]   core : http://127.0.0.1:$core_port/rpc"
echo "[dev:web]   app  : http://localhost:$dev_port"
echo "[dev:web]   open : $connect_url"
if (( attach )); then
  echo "[dev:web]   the browser shares the desktop app's core and signed-in session"
else
  echo "[dev:web]   sign in with one click on a provider; the session persists in the core workspace"
fi
echo

if (( open_browser )); then
  if command -v open >/dev/null 2>&1; then
    open "$connect_url"
  elif command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$connect_url"
  else
    echo "[dev:web] no opener found; visit the URL above." >&2
  fi
else
  echo "[dev:web] --no-browser: point your CDP client at the URL above."
fi

wait "$vite_pid"
