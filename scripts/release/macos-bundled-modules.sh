#!/usr/bin/env bash
# Bundled native modules in a macOS .app, shared by the CI release signer
# (sign-and-notarize-macos.sh) and the local signed build
# (build-macos-signed.sh) so both ship the same thing.
#
# Usage:
#   macos-bundled-modules.sh sign  <app_path> <entitlements_plist> <identity>
#   macos-bundled-modules.sh check <bundled-modules dir>
#
# sign: codesign every Mach-O under Contents/Resources/bundled-modules with
#   hardened runtime and a secure timestamp. Run before sealing the .app.
#   tinybus pins the module *archive* digest, and on macOS stage-modules.mjs
#   replaces that archive with its `.sha256` marker, so signing the extracted
#   files in place does not break admission. Every Mach-O, not only the dylib:
#   tinycomputer ships helper executables beside its library.
#
# check: fail when any Mach-O under the directory — loose, or inside a shipped
#   .tar.gz — lacks what notarization demands: a Developer ID signature,
#   hardened runtime and a secure timestamp. Apple's notary service unpacks
#   nested archives, so archive members count too.
set -euo pipefail

cmd="${1:-}"
case "$cmd" in
  sign)
    APP_PATH="${2:?sign needs <app_path>}"
    ENTITLEMENTS="${3:?sign needs <entitlements_plist>}"
    IDENTITY="${4:?sign needs <identity>}"
    while IFS= read -r -d '' bin; do
      file -b "$bin" | grep -q '^Mach-O' || continue
      echo "[sign]   Signing bundled module: ${bin#"$APP_PATH/Contents/Resources/"}"
      codesign --force --options runtime \
        --entitlements "$ENTITLEMENTS" \
        --sign "$IDENTITY" \
        --timestamp \
        "$bin"
    done < <(find "$APP_PATH/Contents/Resources/bundled-modules" -type f -print0 2>/dev/null)
    ;;
  check)
    ROOT="${2:?check needs <bundled-modules dir>}"
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
    ;;
  *)
    sed -n '2,/^set -euo/p' "$0" | sed '$d' >&2
    exit 1
    ;;
esac
