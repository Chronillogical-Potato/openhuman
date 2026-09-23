#!/usr/bin/env bash
# rss-bench.sh — steady-state RSS of an embedded openhuman_core agent roster
# (#5046): 5 fresh processes x {1, 8} agents, reported against the 20 MiB
# target and 30 MiB hard cap.
#
# Formerly the report-only `rust-rss-bench` CI job; benchmarks run from
# scripts, not CI. The build is a stripped release (thin LTO, one codegen
# unit), so expect a long first compile.
#
# Usage: ./scripts/profile/rss-bench.sh [--out FILE] [--skip-build]
#   --out FILE     raw samples as JSON (default target/profile/rss-bench.json)
#   --skip-build   reuse target/release/rss-bench
set -euo pipefail

cd "$(dirname "$0")/../.."

out="target/profile/rss-bench.json"
build=1
while (($#)); do
  case "$1" in
    --out) out="$2"; shift 2 ;;
    --skip-build) build=0; shift ;;
    -h | --help) sed -n '2,13p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 64 ;;
  esac
done

if ((build)); then
  echo "[rss-bench] building stripped-release rss-bench" >&2
  cargo build --release --features rss-bench --bin rss-bench
  echo "[rss-bench] running fixture tests" >&2
  cargo test --features rss-bench --bin rss-bench
fi

mkdir -p "$(dirname "${out}")"
./target/release/rss-bench --out "${out}"
echo "[rss-bench] raw samples: ${out}" >&2
