use super::*;
use crate::storage::{MemoryStorage, Scope, StorageBackend};

fn keys(byte: u8) -> Arc<dyn KeyProvider> {
    Arc::new(DerivedKeys::new(Zeroizing::new([byte; 32])))
}

fn secrets_in(storage: &MemoryStorage, scope: &str, key: u8) -> DocumentSecrets {
    over(&storage.for_scope(&Scope::new(scope).unwrap()).unwrap(), keys(key))
}

#[test]
fn a_secret_round_trips_from_sync_code() {
    let storage = MemoryStorage::new();
    let secrets = secrets_in(&storage, "local", 7);
    assert!(get_blocking(&secrets, "user:token").unwrap().is_none());
    set_blocking(&secrets, "user:token", b"s3cret").unwrap();
    assert_eq!(
        get_blocking(&secrets, "user:token").unwrap().unwrap().as_slice(),
        b"s3cret"
    );
}

#[test]
fn scopes_and_keys_keep_secrets_apart() {
    let storage = MemoryStorage::new();
    set_blocking(&secrets_in(&storage, "alice", 7), "user:token", b"a").unwrap();
    assert!(
        get_blocking(&secrets_in(&storage, "bob", 7), "user:token")
            .unwrap()
            .is_none(),
        "another scope sees nothing"
    );
    assert!(
        get_blocking(&secrets_in(&storage, "alice", 8), "user:token").is_err(),
        "a different master key cannot decrypt"
    );
}

#[test]
fn the_stored_document_holds_no_plaintext() {
    let storage = MemoryStorage::new();
    set_blocking(&secrets_in(&storage, "local", 7), "user:token", b"plain-value").unwrap();
    let docs = storage
        .for_scope(&Scope::new("local").unwrap())
        .unwrap()
        .documents()
        .clone();
    let page = crate::storage::block_on(async move {
        docs.query(
            tinystoragedrivers::secrets::DEFAULT_COLLECTION,
            &tinystoragedrivers::Query::all(),
        )
        .await
    })
    .unwrap();
    let raw = serde_json::to_string(&page.items[0].doc).unwrap();
    assert!(!raw.contains("plain-value"), "{raw}");
    assert!(raw.contains("enc2:"));
}
