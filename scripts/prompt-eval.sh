#!/usr/bin/env bash
# Tier-2 prompt evals: does a real model follow the agent prompts? Scored,
# costs money, non-deterministic. NEVER gates CI (refuses to run when CI=true).
# Tier 1 (`tests/agent_prompt_comprehension_e2e.rs`) pins the script; only this
# answers whether a model follows it. See docs/prompt-evals.md.
#
# Usage: scripts/prompt-eval.sh [--case <id>] [--bin <openhuman-core>]
#
# Needs a backend credential in OPENHUMAN_BACKEND_SESSION_TOKEN or
# OPENHUMAN_BACKEND_API_KEY (BACKEND_URL optional). Each case runs in its own
# fresh workspace and its own `openhuman-core call` subprocesses, so the
# process-global model override and `AlreadyRunning` never come into it.
#
# Scoring reads artifacts the run already writes:
#   1. hard failure signals — breaker halt (log), [SUBAGENT_INCOMPLETE]
#      (transcripts), trail_off / capped (flows_build result). Any ⇒ unproductive.
#   2. tool calls from <ws>/**/session_raw/*.jsonl: expected, forbidden, repeat caps.
#   3. cost from each transcript's `_meta` line, with the model id beside it.
#   4. optional judge: the production close-verification rubric (cases.json).
# One row per case is appended to target/prompt-eval-runs.jsonl (gitignored).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASES="$ROOT/scripts/prompt-eval/cases.json"
BIN="$ROOT/target/debug/openhuman-core"
OUT="$ROOT/target/prompt-eval-runs.jsonl"
ONLY=""

while [ $# -gt 0 ]; do
  case "$1" in
    --case) ONLY="$2"; shift 2 ;;
    --bin) BIN="$2"; shift 2 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

if [ "${CI:-}" = "true" ]; then
  echo "prompt-eval: refusing to run in CI — tier 2 costs money and is non-deterministic" >&2
  exit 1
fi
if [ -z "${OPENHUMAN_BACKEND_SESSION_TOKEN:-}${OPENHUMAN_BACKEND_API_KEY:-}" ]; then
  echo "prompt-eval: set OPENHUMAN_BACKEND_SESSION_TOKEN or OPENHUMAN_BACKEND_API_KEY" >&2
  exit 2
fi
[ -x "$BIN" ] || { echo "prompt-eval: no binary at $BIN (cargo build --bin openhuman-core)" >&2; exit 2; }
mkdir -p "$(dirname "$OUT")"

ids=$(python3 -c 'import json,sys; print("\n".join(c["id"] for c in json.load(open(sys.argv[1]))["cases"]))' "$CASES")
[ -n "$ONLY" ] && { echo "$ids" | grep -qx "$ONLY" || { echo "no case $ONLY" >&2; exit 2; }; ids="$ONLY"; }

total_usd=0
for id in $ids; do
  ws="$(mktemp -d -t prompt-eval)"
  printf 'chat_onboarding_completed = true\n\n[secrets]\nencrypt = false\n' > "$ws/config.toml"
  export OPENHUMAN_WORKSPACE="$ws" OPENHUMAN_KEYRING_BACKEND=file RUST_LOG="${RUST_LOG:-info}"

  core() { "$BIN" call --method "$1" --params "$2" 2>>"$ws/core.log"; }

  # `call` does not run the server's boot-env credential seeding, so install it.
  # ponytail: the credential rides argv (visible in `ps`) — `call` takes params
  # no other way; fine on a dev machine, add a --params-file before a shared host.
  if [ -n "${OPENHUMAN_BACKEND_API_KEY:-}" ]; then
    cred=$(python3 -c 'import json,os; print(json.dumps({"token": os.environ["OPENHUMAN_BACKEND_API_KEY"], "kind": "api-key"}))')
  else
    cred=$(python3 -c 'import json,os; print(json.dumps({"token": os.environ["OPENHUMAN_BACKEND_SESSION_TOKEN"], "kind": "session"}))')
  fi
  core openhuman.auth_set_credential "$cred" >/dev/null

  entry=$(python3 -c 'import json,sys; c=[c for c in json.load(open(sys.argv[1]))["cases"] if c["id"]==sys.argv[2]][0]; print(c["entry"]); print(c["message"])' "$CASES" "$id")
  kind=$(echo "$entry" | head -1); message=$(echo "$entry" | tail -n +2)
  case "$kind" in
    flows_build) method=openhuman.flows_build
      params=$(python3 -c 'import json,sys; print(json.dumps({"mode": "create", "instruction": sys.argv[1]}))' "$message") ;;
    agent_chat) method=openhuman.agent_chat
      params=$(python3 -c 'import json,sys; print(json.dumps({"message": sys.argv[1]}))' "$message") ;;
    *) echo "case $id: unknown entry $kind" >&2; exit 2 ;;
  esac

  echo "── $id ($method) workspace=$ws" >&2
  started=$(date +%s)
  core "$method" "$params" > "$ws/result.json" || echo "case $id: $method exited non-zero (scored anyway)" >&2
  elapsed=$(( $(date +%s) - started ))

  # Judge in the same workspace (same credential), as a tool-less chat call.
  judge_prompt=$(python3 "$ROOT/scripts/prompt-eval/score.py" judge-prompt "$CASES" "$id" "$ws")
  if [ -n "$judge_prompt" ]; then
    jp=$(python3 -c 'import json,sys; print(json.dumps({"message": sys.stdin.read()}))' <<<"$judge_prompt")
    core openhuman.agent_chat_simple "$jp" > "$ws/judge.json" || true
  fi

  row=$(python3 "$ROOT/scripts/prompt-eval/score.py" score "$CASES" "$id" "$ws" "$elapsed")
  echo "$row" >> "$OUT"
  echo "$row" | python3 -c 'import json,sys; r=json.load(sys.stdin); print("%s: %s score=%s usd=%.4f models=%s failures=%s" % (r["case"], "PASS" if r["pass"] else "FAIL", r["score"], r["usd"], r["models"], r["failures"]))'
  total_usd=$(python3 -c 'import json,sys; print(float(sys.argv[1]) + json.loads(sys.argv[2])["usd"])' "$total_usd" "$row")
done
echo "total USD: $total_usd   (rows appended to $OUT)"
