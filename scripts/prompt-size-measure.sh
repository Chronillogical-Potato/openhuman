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
  # Pin the native tool dialect: under a text dialect (`python`, the default
  # since #6436) the tool catalogue is rendered into the system prompt and
  # would be counted twice, once as prompt bytes and once in the tools column.
  # The prompt column is the prose the agent pays for regardless of dialect;
  # the tools column tracks the catalogue.
  env -u OPENHUMAN_HOME HOME="$TMP/home" RUST_LOG=error \
    OPENHUMAN_TOOL_DISPATCHER=auto \
    "$BIN" agent prompt-size --workspace "$TMP/workspace" --hermetic --json
fi
