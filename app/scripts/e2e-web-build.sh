#!/usr/bin/env bash

set -euo pipefail

APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$APP_DIR/.." && pwd)"
cd "$APP_DIR"

RUSTC_BIN="$(command -v rustc)"
CARGO_BIN="${CARGO_BIN:-$(dirname "$RUSTC_BIN")/cargo}"
if [ ! -x "$CARGO_BIN" ]; then
  CARGO_BIN="$(command -v cargo)"
fi

RUST_HOST_TRIPLE="${RUST_HOST_TRIPLE:-$("$RUSTC_BIN" -vV | awk '/^host: / { print $2 }')}"
E2E_WEB_CORE_TARGET_DIR="${E2E_WEB_CORE_TARGET_DIR:-$REPO_ROOT/target/e2e-web-${RUST_HOST_TRIPLE}}"

# Preserve explicit harness ports before loading a developer .env. That file
# may configure normal development, but must not change an E2E bundle's baked
# endpoint or its E2E-only affordances.
E2E_MOCK_PORT="${E2E_MOCK_PORT:-18473}"
OPENHUMAN_CORE_PORT="${OPENHUMAN_CORE_PORT:-17788}"

if [ -f "$REPO_ROOT/.env" ]; then
  # shellcheck source=/dev/null
  source "$REPO_ROOT/scripts/load-dotenv.sh"
fi

# Apply E2E settings after .env so it cannot produce a non-E2E bundle with a
# valid marker. Keep this immediately before the build that consumes them.
export VITE_BACKEND_URL="http://127.0.0.1:${E2E_MOCK_PORT}"
export VITE_OPENHUMAN_TARGET="web"
export VITE_OPENHUMAN_E2E_DEFAULT_CORE_MODE="cloud"
export VITE_OPENHUMAN_E2E_RESTART_APP_AS_RELOAD="true"
export VITE_OPENHUMAN_CORE_RPC_URL="http://127.0.0.1:${OPENHUMAN_CORE_PORT}/rpc"
export VITE_CHAT_ATTACHMENTS="true"

echo "Building web E2E bundle with backend ${VITE_BACKEND_URL}"
# Drop all build markers before compiling. A failed `build:web` leaves the
# preceding dist-web intact, and the session must not accept that stale bundle.
rm -f "$APP_DIR/dist-web/.openhuman-e2e-bundle" "$APP_DIR/dist-web/.e2e-build-ports.json"
pnpm run build:web
# Mark dist-web as an E2E bundle for e2e-web-session.sh. `pnpm build:web` on its
# own compiles in the wrong backend and none of the E2E affordances, and Vite
# empties dist-web on every build, so any later non-E2E build removes this
# marker and the session refuses that bundle instead of serving it (#5920).
# The recorded values are for diagnosing a bundle, not read back.
cat >"$APP_DIR/dist-web/.openhuman-e2e-bundle" <<MARKER
VITE_BACKEND_URL=${VITE_BACKEND_URL}
VITE_OPENHUMAN_TARGET=${VITE_OPENHUMAN_TARGET}
VITE_OPENHUMAN_E2E_DEFAULT_CORE_MODE=${VITE_OPENHUMAN_E2E_DEFAULT_CORE_MODE}
VITE_OPENHUMAN_E2E_RESTART_APP_AS_RELOAD=${VITE_OPENHUMAN_E2E_RESTART_APP_AS_RELOAD}
VITE_OPENHUMAN_CORE_RPC_URL=${VITE_OPENHUMAN_CORE_RPC_URL}
VITE_CHAT_ATTACHMENTS=${VITE_CHAT_ATTACHMENTS}
MARKER
# Also retain the ports baked into the bundle. The session validates this
# contract before serving: a web bundle cannot change its backend at runtime.
cat >"$APP_DIR/dist-web/.e2e-build-ports.json" <<JSON
{
  "e2e_mock_port": "${E2E_MOCK_PORT:-18473}",
  "openhuman_core_port": "${OPENHUMAN_CORE_PORT:-17788}",
  "vite_backend_url": "${VITE_BACKEND_URL}"
}
JSON
echo "Building standalone openhuman-core for web E2E into ${E2E_WEB_CORE_TARGET_DIR}..."
# A bare core build uses the contributor feature set, which intentionally
# omits product domains such as voice, web3, documents and crash reporting.
# The web suite exercises the shipped desktop surface, so compile the same
# explicit product gates that the Tauri shell and product CI lanes forward.
PRODUCT_FEATURES="$(bash "$REPO_ROOT/scripts/ci/product-features.sh")"
CARGO_TARGET_DIR="$E2E_WEB_CORE_TARGET_DIR" bash "$REPO_ROOT/scripts/ci-cancel-aware.sh" "$CARGO_BIN" build --manifest-path "$REPO_ROOT/Cargo.toml" --bin openhuman-core --features "$PRODUCT_FEATURES"
