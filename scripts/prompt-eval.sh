#!/usr/bin/env bash
# Tier-2 prompt evals: does a real model follow the agent prompts? Scored,
# costs money, non-deterministic. NEVER gates CI (refuses to run when CI=true).
# Tier 1 (`tests/agent_prompt_comprehension_e2e.rs`) pins the script; only this
# answers whether a model follows it. See docs/prompt-evals.md.
#
# Usage: scripts/prompt-eval.sh [--case <id>] [--runs N] [--bin <openhuman-core>] [--real-workspace]
#
# --runs N repeats every case N times (default 1). One run is a sample, not a
# baseline: every run is its own row (case + ts + run + models), never averaged.
#
# Default (hermetic): each case gets a fresh `mktemp -d` workspace and needs a
# credential in OPENHUMAN_BACKEND_SESSION_TOKEN or OPENHUMAN_BACKEND_API_KEY
# (BACKEND_URL optional). --real-workspace instead runs against the signed-in
# ~/.openhuman, where the core reads its own keyring — no credential handling,
# but it writes into the user's real account, is serial, and the desktop app
# must be quit first. See docs/prompt-evals.md for the cleanup it leaves.
# Either way every case runs in its own `openhuman-core call` subprocesses, so
# the process-global model override and `AlreadyRunning` never come into it.
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
REAL=0
RUNS=1

while [ $# -gt 0 ]; do
  case "$1" in
    --case) ONLY="$2"; shift 2 ;;
    --bin) BIN="$2"; shift 2 ;;
    --real-workspace) REAL=1; shift ;;
    --runs) RUNS="$2"; shift 2 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

if [ "${CI:-}" = "true" ]; then
  echo "prompt-eval: refusing to run in CI — tier 2 costs money and is non-deterministic" >&2
  exit 1
fi
if [ "$REAL" = 1 ]; then
  if pgrep -f "OpenHuman.app|openhuman-core serve|openhuman-core run" >/dev/null; then
    echo "prompt-eval: quit the desktop app / core server first — only one process may own ~/.openhuman" >&2
    exit 2
  fi
elif [ -z "${OPENHUMAN_BACKEND_SESSION_TOKEN:-}${OPENHUMAN_BACKEND_API_KEY:-}" ]; then
  echo "prompt-eval: set OPENHUMAN_BACKEND_SESSION_TOKEN or OPENHUMAN_BACKEND_API_KEY" >&2
  exit 2
fi
[ -x "$BIN" ] || { echo "prompt-eval: no binary at $BIN (cargo build --bin openhuman-core)" >&2; exit 2; }
mkdir -p "$(dirname "$OUT")"
# Every row records the tree the measured binary was built from.
PROMPT_EVAL_BIN_SHA="$(git -C "$(dirname "$BIN")" rev-parse HEAD 2>/dev/null || echo unknown)"
export PROMPT_EVAL_BIN_SHA
echo "binary: $BIN @ $PROMPT_EVAL_BIN_SHA" >&2

ids=$(python3 -c 'import json,sys; print("\n".join(c["id"] for c in json.load(open(sys.argv[1]))["cases"]))' "$CASES")
[ -n "$ONLY" ] && { echo "$ids" | grep -qx "$ONLY" || { echo "no case $ONLY" >&2; exit 2; }; ids="$ONLY"; }

case "$RUNS" in ""|*[!0-9]*|0) echo "--runs needs a positive integer" >&2; exit 2 ;; esac

total_usd=0
for id in $ids; do
for run in $(seq 1 "$RUNS"); do
  # $ws always holds this case's own artifacts (result, logs, judge verdict).
  ws="$(mktemp -d -t prompt-eval)"
  export RUST_LOG="${RUST_LOG:-info}"
  if [ "$REAL" = 1 ]; then
    # Transcripts land in the real workspace; score only files this case wrote.
    export PROMPT_EVAL_TRANSCRIPT_ROOT="$HOME/.openhuman" PROMPT_EVAL_SINCE="$(date +%s)"
  else
    printf 'chat_onboarding_completed = true\n\n[secrets]\nencrypt = false\n' > "$ws/config.toml"
    export OPENHUMAN_WORKSPACE="$ws" OPENHUMAN_KEYRING_BACKEND=file
  fi

  core() { "$BIN" call --method "$1" --params "$2" 2>>"$ws/core.log"; }

  # `call` does not run the server's boot-env credential seeding, so install it.
  # ponytail: the credential rides argv (visible in `ps`) — `call` takes params
  # no other way; fine on a dev machine, add a --params-file before a shared host.
  if [ "$REAL" = 0 ]; then
    if [ -n "${OPENHUMAN_BACKEND_API_KEY:-}" ]; then
      cred=$(python3 -c 'import json,os; print(json.dumps({"token": os.environ["OPENHUMAN_BACKEND_API_KEY"], "kind": "api-key"}))')
    else
      cred=$(python3 -c 'import json,os; print(json.dumps({"token": os.environ["OPENHUMAN_BACKEND_SESSION_TOKEN"], "kind": "session"}))')
    fi
    core openhuman.auth_set_credential "$cred" >/dev/null
  fi

  entry=$(python3 -c 'import json,sys; c=[c for c in json.load(open(sys.argv[1]))["cases"] if c["id"]==sys.argv[2]][0]; print(c["entry"]); print(c["message"])' "$CASES" "$id")
  kind=$(echo "$entry" | head -1); message=$(echo "$entry" | tail -n +2)
  case "$kind" in
    flows_build) method=openhuman.flows_build
      params=$(python3 -c 'import json,sys; print(json.dumps({"mode": "create", "instruction": sys.argv[1]}))' "$message") ;;
    agent_chat) method=openhuman.agent_chat
      # A fresh thread per run: with no thread_id, agent_chat resumes the
      # newest orchestrator transcript, so run 2 would see run 1.
      params=$(python3 -c 'import json,sys,time; print(json.dumps({"message": sys.argv[1], "thread_id": "prompt-eval-%s-r%s-%d" % (sys.argv[2], sys.argv[3], time.time())}))' "$message" "$id" "$run") ;;
    *) echo "case $id: unknown entry $kind" >&2; exit 2 ;;
  esac

  echo "── $id run $run/$RUNS ($method) workspace=$ws" >&2

  # Precondition: a read-only RPC checked just before the run. A failure skips
  # the run and records why — a missing connection must not read as a prompt
  # failure.
  pre=$(python3 -c 'import json,sys; c=[c for c in json.load(open(sys.argv[1]))["cases"] if c["id"]==sys.argv[2]][0]; p=c.get("precondition"); print(p["method"] + "\t" + json.dumps(p.get("params", {})) if p else "")' "$CASES" "$id")
  if [ -n "$pre" ]; then
    core "${pre%%$'\t'*}" "${pre#*$'\t'}" > "$ws/precondition.json" || true
    if ! python3 "$ROOT/scripts/prompt-eval/score.py" precondition "$CASES" "$id" "$ws/precondition.json" > "$ws/precondition_result.json"; then
      python3 -c 'import json,sys,time; print(json.dumps({"ts": time.strftime("%Y-%m-%dT%H:%M:%S%z"), "case": sys.argv[1], "run": int(sys.argv[2]), "skipped": "precondition failed", "precondition": json.load(open(sys.argv[3])), "usd": 0.0, "workspace": sys.argv[4]}))' "$id" "$run" "$ws/precondition_result.json" "$ws" >> "$OUT"
      echo "$id run $run: SKIPPED — precondition failed: $(cat "$ws/precondition_result.json")" >&2
      continue
    fi
  fi
  # Wall clock of the agent call, measured here, not read from artifacts:
  # transcript timestamps are stamped at persist time, not turn boundaries.
  started=$(python3 -c 'import time; print(time.time())')
  core "$method" "$params" > "$ws/result.json" || echo "case $id: $method exited non-zero (scored anyway)" >&2
  elapsed=$(python3 -c 'import sys,time; print(round(time.time() - float(sys.argv[1]), 2))' "$started")

  # Judge in the same workspace (same credential), as a tool-less chat call.
  judge_prompt=$(python3 "$ROOT/scripts/prompt-eval/score.py" judge-prompt "$CASES" "$id" "$ws")
  if [ -n "$judge_prompt" ]; then
    jp=$(python3 -c 'import json,sys; print(json.dumps({"message": sys.stdin.read()}))' <<<"$judge_prompt")
    core openhuman.agent_chat_simple "$jp" > "$ws/judge.json" || true
  fi

  row=$(python3 "$ROOT/scripts/prompt-eval/score.py" score "$CASES" "$id" "$ws" "$elapsed" "$run")
  echo "$row" >> "$OUT"
  echo "$row" | python3 -c 'import json,sys; r=json.load(sys.stdin); print("%s run %s: %s score=%s usd=%.4f models=%s failures=%s" % (r["case"], r["run"], "PASS" if r["pass"] else "FAIL", r["score"], r["usd"], r["models"], r["failures"]))'
  total_usd=$(python3 -c 'import json,sys; print(float(sys.argv[1]) + json.loads(sys.argv[2])["usd"])' "$total_usd" "$row")
done
done
echo "total USD: $total_usd   (rows appended to $OUT)"
if [ "$REAL" = 1 ]; then
  echo "real workspace: clean up the test threads, plus each case's \"writes\" in $CASES" >&2
fi
