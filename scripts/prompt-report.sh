#!/usr/bin/env bash
# Print every agent's fixed per-turn prefix — system prompt plus advertised tool
# schemas — one row per agent, largest first, then their sum (a size measure,
# not a cost: each prefix is paid only on the turns that agent runs).
#
# Report-only: nothing here fails on a number. The ratchet that does is
# `scripts/check-prompt-budget.sh`, which measures through the same CLI, so the
# two never disagree about a byte.
#
# Usage: scripts/prompt-report.sh [--workspace <dir>]
#
#   (default)          Hermetic: a fresh empty workspace and config, so the
#                      numbers describe the repo, not whoever is logged in.
#   --workspace <dir>  Measure a real, signed-in workspace instead. The only way
#                      to see `integrations_agent`, which renders once per
#                      *connected* toolkit and so has nothing to render
#                      hermetically.
#
# Units are bytes. `~tok` is bytes / `EST_BYTES_PER_TOKEN` (agent/debug/
# prompt_size.rs, read at run time), the same reading aid `prompt-size` prints —
# a divisor, not a tokenizer. A real token count is true for one model only, and the fleet
# spans several.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

REAL_WORKSPACE=""
while (( $# )); do
  case "$1" in
    --workspace) REAL_WORKSPACE="${2:?--workspace needs a directory}"; shift 2 ;;
    *) echo "unknown flag: $1" >&2; exit 64 ;;
  esac
done

# Always build: cargo is a no-op when the binary is current, and it is the only
# thing that notices a binary built from older source or a different feature
# set. A default-feature build silently drops `presentation_agent`.
BIN="${CARGO_TARGET_DIR:-target}/debug/openhuman-core"
cargo build --quiet --manifest-path Cargo.toml --bin openhuman-core \
  --features "$(bash scripts/ci/product-features.sh)" >&2

# The divisor is read from the Rust constant so the two cannot drift.
TOK="$(grep -oE 'EST_BYTES_PER_TOKEN: usize = [0-9]+' \
  crates/openhuman-core/src/agent/debug/prompt_size.rs | grep -oE '[0-9]+$')" \
  || { echo "EST_BYTES_PER_TOKEN not found in prompt_size.rs" >&2; exit 1; }

if [[ -n "$REAL_WORKSPACE" ]]; then
  echo "[prompt-report] measuring signed-in workspace $REAL_WORKSPACE" >&2
  measured="$(RUST_LOG=error "$BIN" agent prompt-size --workspace "$REAL_WORKSPACE" --json)"
else
  # `--hermetic` puts config.toml beside the workspace dir (its parent), so the
  # workspace must be a child of the temp dir — see check-prompt-budget.sh.
  # HOME is emptied and OPENHUMAN_HOME dropped for the same reason as there:
  # `--hermetic` does not stop skill and agent-definition discovery reading
  # the user's ~/.openhuman (or $OPENHUMAN_HOME).
  TMP="$(mktemp -d "${TMPDIR:-/tmp}/openhuman-prompt-report.XXXXXX")"
  trap 'rm -rf "$TMP"' EXIT
  mkdir -p "$TMP/home" "$TMP/workspace"
  echo "[prompt-report] measuring against hermetic workspace $TMP" >&2
  measured="$(env -u OPENHUMAN_HOME HOME="$TMP/home" RUST_LOG=error "$BIN" agent prompt-size --workspace "$TMP/workspace" --hermetic --json)"
fi

TOK="$TOK" python3 - "$measured" <<'PY'
import json, os, sys

TOK = int(os.environ["TOK"])  # EST_BYTES_PER_TOKEN
rows = []
for r in json.loads(sys.argv[1])["agents"]:
    name = r["agent"] + (f"[{r['toolkit']}]" if r.get("toolkit") else "")
    worst = max(r["tools"], key=lambda t: t["bytes"], default=None)
    rows.append((name, r["prompt_bytes"], r["tool_bytes"], r["fixed_prefix_bytes"],
                 f"{worst['name']} ({worst['bytes']})" if worst else "-"))
rows.sort(key=lambda row: -row[3])

fmt = "{:<34} {:>9} {:>9} {:>9} {:>8}  {}"
print(fmt.format("agent", "prompt B", "tools B", "fixed B", "~tok", "worst tool (B)"))
for name, p, t, f, w in rows:
    print(fmt.format(name, p, t, f, f // TOK, w))
tp, tt, tf = (sum(row[i] for row in rows) for i in (1, 2, 3))
# A sum no single turn pays: each prefix is paid only when that agent runs.
print(fmt.format(f"sum of {len(rows)} (not a turn cost)", tp, tt, tf, tf // TOK, ""))

if not any(row[0].startswith("integrations_agent") for row in rows):
    print("integrations_agent — not measurable hermetically; run with "
          "--workspace ~/.openhuman/workspace to measure per connected toolkit")
PY
