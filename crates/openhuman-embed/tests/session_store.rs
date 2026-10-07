//! End-to-end proof that a host session store owns every conversation.
//!
//! A stateless runtime with an [`InMemorySessionStores`] runs two agents
//! against a `wiremock` provider. Their transcripts, journal and run status
//! must land in the store, each agent's apart from the other's; a reopened
//! agent must resume its thread *from the store*; and nothing durable may be
//! written under the runtime's scratch workspace.
//!
//! One `#[test]` because a runtime claims a process-wide slot.

mod common;

use std::collections::HashSet;
use std::sync::Arc;

use common::{chat_requests, offline_config, provider, runtime, stub_backend};
use openhuman_embed::session_store::{session_store_conformance, Store};
use openhuman_embed::{
    Access, AgentSpec, InMemorySessionStores, Provider, Runtime, RuntimeError,
    SessionStoreProvider, Workspace,
};

/// Run status records, as the journal writes them.
const STATUS_NS: &str = "run_status";

/// Every path under `root`, relative to it.
fn files_under(root: &std::path::Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path.clone());
            }
            found.push(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    found
}

#[test]
fn a_stateless_runtime_keeps_every_conversation_in_its_session_store() {
    let _ = env_logger::builder().is_test(true).try_init();
    runtime().block_on(async {
        tokio::spawn(async move {
            // No store, no stateless runtime — and the failed build releases
            // the process slot, so the next build below succeeds.
            let refused = Runtime::builder()
                .config(offline_config())
                .workspace(Workspace::stateless())
                .build()
                .await;
            assert!(
                matches!(refused, Err(RuntimeError::NoSessionStore)),
                "a stateless runtime needs a session store"
            );

            let backend = stub_backend().await;
            let model = provider("noted").await;
            let store = Arc::new(InMemorySessionStores::new());
            let mut config = offline_config();
            // On by default in the product: it must stand down by itself once
            // a host store owns the transcripts.
            config.agent.session_dual_write = true;
            let runtime = Runtime::builder()
                .config(config)
                .workspace(Workspace::stateless())
                .backend_url(backend.uri())
                .session_store(store.clone())
                .build()
                .await
                .expect("a stateless runtime with a store builds");
            assert!(openhuman_core::agent::session_store::is_installed());
            let scratch = runtime.root_dir().to_path_buf();

            let spec = |id: &str| {
                AgentSpec::new(id)
                    .provider(
                        Provider::openai_compatible(format!("{}/v1", model.uri()), "sk-test")
                            .model("store-model"),
                    )
                    .access(Access::readonly())
            };

            // ── asha: two turns on one thread ─────────────────────────────
            let asha = runtime.agent(spec("asha")).expect("asha instantiates");
            let ravi = runtime.agent(spec("ravi")).expect("ravi instantiates");
            // Runtime initialization and agent construction create required
            // configuration/scaffolding files. Snapshot that legitimate
            // baseline, then ensure turns add no durable files of any kind.
            let baseline: HashSet<_> = files_under(&scratch).into_iter().collect();
            let first = asha
                .turn("remember the table is for two")
                .send()
                .await
                .expect("first turn");
            assert_eq!(first.reply, "noted");
            let thread = first.session_id.clone();
            asha.turn("and at eight")
                .session(&thread)
                .send()
                .await
                .expect("second turn");
            drop(asha);

            // ── reopened, asha resumes from the store ─────────────────────
            let asha = runtime.agent(spec("asha")).expect("asha reopens");
            asha.turn("what did I say?")
                .session(&thread)
                .send()
                .await
                .expect("resumed turn");
            let requests = chat_requests(&model).await;
            let resumed =
                String::from_utf8_lossy(&requests.last().expect("a request").body).into_owned();
            assert!(
                resumed.contains("remember the table is for two")
                    && resumed.contains("and at eight"),
                "the reopened agent replays its thread from the store"
            );

            // ── ravi: a different agent ───────────────────────────────────
            ravi.turn("hello").send().await.expect("ravi's turn");

            // Asha's transcript is in her stores, and only hers.
            let asha_stores = store.for_agent("asha");
            let transcript = asha_stores
                .transcripts
                .root_for_thread(&thread)
                .expect("asha's thread is in her store")
                .read_session()
                .expect("readable")
                .expect("written");
            assert!(transcript
                .messages
                .iter()
                .any(|message| message.content.contains("table is for two")));
            assert!(
                store
                    .for_agent("ravi")
                    .transcripts
                    .root_for_thread(&thread)
                    .is_none(),
                "another agent cannot reach asha's thread"
            );
            // The turn journal's run status went to the agent's own store.
            let runs = asha_stores
                .kv
                .list(STATUS_NS)
                .await
                .expect("status records list");
            assert!(!runs.is_empty(), "asha's runs are recorded in her store");

            // Nothing durable reached the scratch workspace.
            let written: Vec<_> = files_under(&scratch)
                .into_iter()
                .filter(|path| !baseline.contains(path))
                .collect();
            assert!(
                written.is_empty(),
                "a stateless workspace must remain empty, found: {written:?}"
            );

            drop((asha, ravi));
            drop(runtime);
            assert!(
                !openhuman_core::agent::session_store::is_installed(),
                "the store is removed with the runtime"
            );
            assert!(!scratch.exists(), "the scratch workspace is removed");

            // The store a runtime used still meets the port's contract.
            session_store_conformance(store.as_ref()).await;
        })
        .await
        .expect("test task");
    });
}
