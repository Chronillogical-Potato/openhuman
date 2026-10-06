#!/usr/bin/env bash
# Fail when any Mach-O under a bundled-modules directory — loose, or inside a
# shipped .tar.gz release archive — lacks what notarization demands: a
# Developer ID signature, hardened runtime and a secure timestamp. Apple's
# notary service unpacks nested archives, so archive members count too.
#
# Usage: check-macos-bundled-signatures.sh <bundled-modules dir>
set -euo pipefail

ROOT="${1:?Usage: check-macos-bundled-signatures.sh <bundled-modules dir>}"
[ -d "$ROOT" ] || { echo "[sign-check] no bundled modules at $ROOT"; exit 0; }

SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
BAD=0

check_tree() { # <dir> <label prefix>
  while IFS= read -r -d '' f; do
    file -b "$f" | grep -q '^Mach-O' || continue
    info="$(codesign -dvv "$f" 2>&1 || true)"
    problems=""
    grep -q '^Authority=Developer ID Application' <<<"$info" || problems+=" no-developer-id"
    grep -q '^Timestamp=' <<<"$info" || problems+=" no-timestamp"
    grep -q '^CodeDirectory.*flags=.*runtime' <<<"$info" || problems+=" no-hardened-runtime"
    if [ -n "$problems" ]; then
      echo "[sign-check] UNSIGNED for notarization:$problems: $2${f#"$1"/}"
      BAD=1
    fi
  done < <(find "$1" -type f -print0)
}

check_tree "$ROOT" ""
while IFS= read -r -d '' archive; do
  dest="$SCRATCH/$(basename "$archive")"
  mkdir -p "$dest"
  tar -xzf "$archive" -C "$dest"
  check_tree "$dest" "${archive#"$ROOT"/}/"
done < <(find "$ROOT" -name '*.tar.gz' -type f -print0)

if [ "$BAD" -ne 0 ]; then
  echo "[sign-check] ERROR: notarization would reject the Mach-O files above" >&2
  exit 1
fi
echo "[sign-check] every Mach-O under $ROOT is notarization-ready"
