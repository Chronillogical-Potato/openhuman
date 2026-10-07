set -x
F="$(bash scripts/ci/product-features.sh)"
cargo check --tests -p openhuman -p openhuman-cli --no-default-features --features "$F"
