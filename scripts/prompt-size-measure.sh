#!/usr/bin/env bash
# Build the product binary and emit the canonical prompt-size JSON.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
WORKSPACE_ARG=""
while (( $# )); do
  case "$1" in
    --workspace) WORKSPACE_ARG="${2:?--workspace needs a directory}"; shift 2 ;;
    *) echo "unknown flag: $1" >&2; exit 64 ;;
  esac
done
TARGET_DIR="${CARGO_TARGET_DIR:-target}"
BIN="$TARGET_DIR/debug/openhuman-core"
echo "[prompt-size] building openhuman-core …" >&2
cargo build --quiet --manifest-path Cargo.toml --bin openhuman-core \
  --features "$(bash scripts/ci/product-features.sh)" >&2
if [[ -n "$WORKSPACE_ARG" ]]; then
  RUST_LOG=error "$BIN" agent prompt-size --workspace "$WORKSPACE_ARG" --json
else
  TMP="$(mktemp -d "${TMPDIR:-/tmp}/openhuman-prompt-size.XXXXXX")"
  trap 'rm -rf "$TMP"' EXIT
  mkdir -p "$TMP/home" "$TMP/workspace"
  env -u OPENHUMAN_HOME HOME="$TMP/home" RUST_LOG=error \
    "$BIN" agent prompt-size --workspace "$TMP/workspace" --hermetic --json
fi
