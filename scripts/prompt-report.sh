#!/usr/bin/env bash
# Print every agent's fixed per-turn prefix — system prompt plus advertised tool
# schemas — one row per agent, largest first, with a fleet total.
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
# Units are bytes. `~tok` is bytes / 4, the same reading aid `prompt-size`
# prints (`EST_BYTES_PER_TOKEN` in agent/debug/prompt_size.rs) — a divisor, not
# a tokenizer. A real token count is true for one model only, and the fleet
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

BIN="${CARGO_TARGET_DIR:-target}/debug/openhuman-core"
if [[ ! -x "$BIN" ]]; then
  echo "[prompt-report] building openhuman-core …" >&2
  cargo build --manifest-path Cargo.toml --bin openhuman-core \
    --features "$(bash scripts/ci/product-features.sh)" >&2
fi

if [[ -n "$REAL_WORKSPACE" ]]; then
  echo "[prompt-report] measuring signed-in workspace $REAL_WORKSPACE" >&2
  measured="$(RUST_LOG=error "$BIN" agent prompt-size --workspace "$REAL_WORKSPACE" --json)"
else
  # `--hermetic` puts config.toml beside the workspace dir (its parent), so the
  # workspace must be a child of the temp dir — see check-prompt-budget.sh.
  TMP="$(mktemp -d "${TMPDIR:-/tmp}/openhuman-prompt-report.XXXXXX")"
  trap 'rm -rf "$TMP"' EXIT
  echo "[prompt-report] measuring against hermetic workspace $TMP" >&2
  measured="$(RUST_LOG=error "$BIN" agent prompt-size --workspace "$TMP/workspace" --hermetic --json)"
fi

python3 - "$measured" <<'PY'
import json, sys

TOK = 4  # EST_BYTES_PER_TOKEN
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
print(fmt.format(f"TOTAL ({len(rows)} rows)", tp, tt, tf, tf // TOK, ""))

if not any(row[0].startswith("integrations_agent") for row in rows):
    print("integrations_agent — not measurable hermetically; run with "
          "--workspace ~/.openhuman/workspace to measure per connected toolkit")
PY
