use super::*;
use std::sync::Arc;
use tinymemory_api::conformance::ReferenceEngine;
use tinymemory_api::{MemoryEngine, MemoryMeta, Role, SourceKind, StoreItem, Turn};

fn conversation(thread: &str) -> StoreItem {
    let mut meta = MemoryMeta::from_source(SourceKind::Conversation, Some(thread.to_string()));
    meta.thread_id = Some(thread.to_string());
    StoreItem::Conversation {
        turns: vec![Turn::new(Role::User, format!("hello from {thread}"))],
        meta,
    }
}

#[tokio::test]
async fn clears_only_the_channels_conversations() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut config = Config::default();
    config.workspace_dir = tmp.path().to_path_buf();
    let engine = Arc::new(ReferenceEngine::new());
    crate::memory::engine::install_test_engine(&config.workspace_dir, engine.clone());
    engine.store(conversation("discord-1")).await.unwrap();
    engine.store(conversation("telegram-1")).await.unwrap();
    crate::memory::channels::record(&config.workspace_dir, "Discord", "discord-1");
    crate::memory::channels::record(&config.workspace_dir, "telegram", "telegram-1");

    assert_eq!(clear_channel_memory(&config, "discord").await.unwrap(), 1);
    assert_eq!(engine.len(), 1);
    assert!(crate::memory::channels::threads_of(&config.workspace_dir, "discord").is_empty());
}

#[tokio::test]
async fn memory_off_clears_nothing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut config = Config::default();
    config.workspace_dir = tmp.path().to_path_buf();
    config.memory.engine = String::new();
    assert_eq!(clear_channel_memory(&config, "discord").await.unwrap(), 0);
}
