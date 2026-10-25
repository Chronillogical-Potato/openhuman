#!/usr/bin/env bash
set -euo pipefail

# Native integration test loads the published archive through the compiled
# registry digest. The fixture is intentionally isolated from ordinary tests.
export OPENHUMAN_TEST_SECURITY_FIXTURE_DIR="${OPENHUMAN_TEST_SECURITY_FIXTURE_DIR:-$HOME/.cache/openhuman-security-fixtures}"
mkdir -p "$OPENHUMAN_TEST_SECURITY_FIXTURE_DIR"

core_features="$(scripts/ci/product-features.sh | sed 's/,os-keyring//' | sed 's/,security-module//')"

security_pin="$(git -C vendor/tinysecurity rev-parse HEAD)"
release_source="$(git -C vendor/tinysecurity rev-parse b62ad69ce28bc3178ad91e12f3074f21007055c4)"
printf 'TinySecurity fork pin: %s\nPublished v0.17.1 source: %s\n' "$security_pin" "$release_source"
git -C vendor/tinysecurity merge-base --is-ancestor "$release_source" "$security_pin"

# Prove the library builds without the new gate before exercising it enabled.
bash scripts/ci-cancel-aware.sh cargo check --manifest-path Cargo.toml -p openhuman \
  --no-default-features --features "$core_features"

bash scripts/ci-cancel-aware.sh cargo test --manifest-path Cargo.toml -p openhuman --lib \
  --no-default-features --features "$core_features,security-module" \
  modules::security::tests::native_fixture_loads_the_pinned_release_and_reads_policy_info \
  -- --ignored --exact --nocapture
