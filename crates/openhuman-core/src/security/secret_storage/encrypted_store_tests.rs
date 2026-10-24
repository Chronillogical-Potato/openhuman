use super::*;
use tempfile::TempDir;

fn hex_encode(data: &[u8]) -> String {
    ::hex::encode(data)
}

fn hex_decode(hex: &str) -> anyhow::Result<Vec<u8>> {
    ::hex::decode(hex).map_err(anyhow::Error::from)
}

#[path = "encrypted_store_crypto_migration_tests.rs"]
mod crypto_migration_tests;
