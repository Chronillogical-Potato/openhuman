#!/usr/bin/env bash
set -euo pipefail

# Native integration test loads the published archive through the compiled
# registry digest. The fixture is intentionally isolated from ordinary tests.
export OPENHUMAN_TEST_SECURITY_FIXTURE_DIR="${OPENHUMAN_TEST_SECURITY_FIXTURE_DIR:-$HOME/.cache/openhuman-security-fixtures}"
mkdir -p "$OPENHUMAN_TEST_SECURITY_FIXTURE_DIR"

core_features="$(scripts/ci/product-features.sh | sed 's/,os-keyring//')"

bash scripts/ci-cancel-aware.sh cargo test --manifest-path Cargo.toml -p openhuman --lib \
  --no-default-features --features "$core_features" \
  modules::security::tests::native_fixture_loads_the_pinned_release_and_reads_policy_info \
  -- --ignored --exact --nocapture
