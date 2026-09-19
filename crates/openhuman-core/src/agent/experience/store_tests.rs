//! Unit tests for [`AgentExperienceStore`](super::AgentExperienceStore).
//!
//! A sibling `*_tests.rs` rather than an inline `#[cfg(test)] mod tests`, for
//! two reasons that point the same way. It keeps `store.rs` near the ~500-line
//! guideline, and it is the path both memory ratchets skip
//! (`direct_engine_refs_tests::is_test_path`), which is what lets the two
//! sanitizer regressions below keep constructing a real `UnifiedMemory`. That
//! reference is legitimate: `tinymemory-core` is a dev-dependency after #5560,
//! and a dev-dependency is not linked into the shipped binary — but in
//! `store.rs` it read as an unmigrated production call site and cost an
//! allowlist entry that documented the opposite of the truth.

use super::*;
use crate::agent::experience::types::{AgentExperience, ExperienceOutcome, ExperienceSource};
use crate::memory::tool_memory::test_helpers::MockMemory;
use std::sync::Arc;

fn sample_experience(
    id: &str,
    task_summary: &str,
    tools: Vec<&str>,
    tags: Vec<&str>,
    confidence: f32,
) -> AgentExperience {
    let sequence = tools.iter().map(|tool| (*tool).to_string()).collect();
    AgentExperience {
        id: id.to_string(),
        created_at_ms: 1,
        updated_at_ms: 1,
        source: ExperienceSource::ToolLoop,
        agent_id: Some("orchestrator".into()),
        entrypoint: Some("chat".into()),
        task_fingerprint: format!("fp-{id}"),
        task_summary: task_summary.to_string(),
        tools_used: tools.iter().map(|tool| (*tool).to_string()).collect(),
        tool_sequence: sequence,
        outcome: ExperienceOutcome::Success,
        error_class: None,
        lesson: format!("lesson for {task_summary}"),
        reuse_hint: format!("reuse for {task_summary}"),
        avoid_hint: None,
        confidence,
        tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        payload_hash: None,
        dismissed: false,
    }
}

fn fresh_store() -> (AgentExperienceStore, Arc<MockMemory>) {
    let memory = Arc::new(MockMemory::default());
    (AgentExperienceStore::new(memory.clone()), memory)
}

#[tokio::test]
async fn put_list_and_dismiss_round_trip() {
    let (store, memory) = fresh_store();
    store
        .put(sample_experience(
            "exp_success",
            "search repository docs",
            vec!["grep", "file_read"],
            vec!["docs"],
            0.8,
        ))
        .await
        .unwrap();

    assert!(memory.entries.lock().contains_key(&(
        AGENT_EXPERIENCE_NAMESPACE.into(),
        "experience/exp_success".into()
    )));

    let listed = store.list().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "exp_success");

    let dismissed = store.dismiss("exp_success").await.unwrap();
    assert!(dismissed);
    let listed = store.list().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].dismissed);
}


#[tokio::test]
async fn retrieve_ignores_dismissed_records() {
    let (store, _) = fresh_store();
    store
        .put(sample_experience(
            "exp_dismissed",
            "search repository docs",
            vec!["grep", "file_read"],
            vec!["docs"],
            0.8,
        ))
        .await
        .unwrap();
    store.dismiss("exp_dismissed").await.unwrap();

    let hits = store
        .retrieve(ExperienceQuery {
            query: "search repository docs".into(),
            tools: vec!["grep".into()],
            tags: vec!["docs".into()],
            agent_id: None,
            entrypoint: None,
            max_hits: 5,
        })
        .await
        .unwrap();

    assert!(hits.is_empty());
}
