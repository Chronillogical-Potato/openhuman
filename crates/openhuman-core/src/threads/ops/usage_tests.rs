//! Aggregation tests for `threads::ops::usage`.
//!
//! Every fixture is written with the session crate's own writer
//! (`append_transcript_turn`), never hand-rolled JSONL, so a wire-format change
//! upstream surfaces here as a failure instead of silently diverging from what
//! the core actually persists.

use super::*;
use tinyagents_session::transcript::{
    append_transcript_turn, MessageUsage, TranscriptMessage, TranscriptMeta, TurnUsage,
};

const MODEL: &str = "openrouter/deepseek/deepseek-v4-flash";

fn meta(agent: &str, agent_type: &str, thread_id: Option<&str>) -> TranscriptMeta {
    TranscriptMeta {
        agent_name: agent.to_string(),
        agent_id: Some(agent.to_string()),
        agent_type: Some(agent_type.to_string()),
        dispatcher: "native".into(),
        provider: Some("openhuman".into()),
        model: Some(MODEL.into()),
        created: "2026-09-22T00:00:00Z".into(),
        updated: "2026-09-22T01:00:00Z".into(),
        turn_count: 0,
        // Every fixture leaves the denormalised `_meta` rollups at zero: that is
        // exactly what is on disk for every root transcript written since
        // `33566d382`, and the aggregate must not depend on them.
        input_tokens: 0,
        output_tokens: 0,
        cached_input_tokens: 0,
        charged_amount_usd: 0.0,
        thread_id: thread_id.map(str::to_owned),
        task_id: None,
        session_id: None,
        parent_session_id: None,
    }
}

fn turn_usage(input: u64, output: u64, cached: u64) -> TurnUsage {
    TurnUsage {
        provider: "openhuman".into(),
        model: MODEL.into(),
        usage: MessageUsage {
            input,
            output,
            cached_input: cached,
            context_window: 1_000_000,
            cost_usd: 0.0,
        },
        ts: "2026-09-22T01:00:00Z".into(),
        reasoning_content: None,
        tool_calls: Vec::new(),
        iteration: 1,
    }
}

/// Append `turns` to `session_raw/{stem}.jsonl`, one durable turn per entry,
/// growing the message set the way the runtime does.
fn write_transcript_turns(
    workspace: &Path,
    stem: &str,
    meta: &TranscriptMeta,
    turns: &[(u64, u64, u64)],
) {
    let dir = workspace.join("session_raw");
    std::fs::create_dir_all(&dir).expect("create session_raw");
    let path = dir.join(format!("{stem}.jsonl"));
    let mut persisted: Vec<TranscriptMessage> = Vec::new();
    for (i, (input, output, cached)) in turns.iter().enumerate() {
        let mut next = persisted.clone();
        next.push(TranscriptMessage::new("user", format!("q{i}")));
        next.push(TranscriptMessage::assistant(format!("a{i}")));
        let mut turn_meta = meta.clone();
        turn_meta.turn_count = i + 1;
        append_transcript_turn(
            &path,
            &persisted,
            &next,
            &turn_meta,
            Some(&turn_usage(*input, *output, *cached)),
            Some(&format!("req-{i}")),
        )
        .expect("append turn");
        persisted = next;
    }
}

/// A thread with no sub-agents: the total is the orchestrator's own spend, and
/// the zero `_meta` rollups are ignored.
#[test]
fn totals_a_thread_with_no_subagents_from_its_turn_records() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let thread = "thread-solo";
    write_transcript_turns(
        tmp.path(),
        "1790000000_orchestrator_solo",
        &meta("orchestrator", "root", Some(thread)),
        &[(10_000, 100, 4_000), (20_000, 200, 8_000)],
    );

    let spend = thread_spend(tmp.path(), thread);

    assert_eq!(spend.root.input_tokens, 30_000);
    assert_eq!(spend.root.output_tokens, 300);
    assert_eq!(spend.root.cached_input_tokens, 12_000);
    assert_eq!(spend.root.turns, 2);
    assert!(
        spend.subagents.is_empty(),
        "no sub-agent transcripts exist, so none may be attributed"
    );
    // The newest turn owns the last-turn view, not the sum of turns.
    assert_eq!(spend.root.last_input_tokens, 20_000);
    assert_eq!(spend.root.last_output_tokens, 200);
}

/// The `thread-37dcc15f` shape from the bug report: a root whose children burned
/// far more than it did, and whose child transcripts carry a **worker** thread
/// id rather than the parent's.
///
/// This is the regression. Selecting children by `_meta.thread_id` — what
/// `read_thread_usage_summary` does — attributed 0 of the 397,423 child input
/// tokens to the thread, so ~88% of its spend was invisible (#6460).
#[test]
fn counts_subagents_whose_transcripts_carry_a_worker_thread_id() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let thread = "thread-37dcc15f";
    let root_stem = "1789765020_orchestrator_thread-37dcc";
    write_transcript_turns(
        tmp.path(),
        root_stem,
        &meta("orchestrator", "root", Some(thread)),
        &[(16_884, 34, 0)],
    );
    // Both children name their own worker thread, never `thread`.
    write_transcript_turns(
        tmp.path(),
        &format!("{root_stem}__1789766110_researcher_sub-a"),
        &meta(
            "researcher",
            "subagent",
            Some("worker-0aa53709-436d-4566-8798-3629beb11723"),
        ),
        &[(180_295, 8_000, 0)],
    );
    write_transcript_turns(
        tmp.path(),
        &format!("{root_stem}__1789766852_researcher_sub-b"),
        &meta(
            "researcher",
            "subagent",
            Some("worker-6b2d242f-c1b1-49ef-8b74-0a7ebeda4afc"),
        ),
        &[(217_128, 8_873, 0)],
    );

    let spend = thread_spend(tmp.path(), thread);

    assert_eq!(spend.root.input_tokens, 16_884, "orchestrator's own spend");
    let (researcher, runs) = spend
        .subagents
        .get("researcher")
        .expect("both children group under their archetype despite the worker thread ids");
    assert_eq!(*runs, 2, "two separate delegations, two runs");
    assert_eq!(
        researcher.input_tokens, 397_423,
        "both children's input must be attributed to the parent thread"
    );
    assert_eq!(researcher.output_tokens, 16_873);

    // The thread total counts every token exactly ONCE. The pre-fix arithmetic
    // would reach 414,307 + 397,423 had the root record also carried its
    // children, which is the double count this design removes.
    let total = spend.root.input_tokens + researcher.input_tokens;
    assert_eq!(total, 414_307);
}

/// A child's spend is counted from the child's own transcript, so a delegation
/// whose usage never reached the parent's in-turn ledger (#6459, the default
/// async path) is still counted — and counted once, not twice.
#[test]
fn counts_each_transcript_once_regardless_of_the_parent_ledger() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let thread = "thread-ledger";
    let root_stem = "1790000001_orchestrator_ledger";
    // The root records ONLY its own spend — the contract
    // `session_host::codec::turn_usage` now guarantees.
    write_transcript_turns(
        tmp.path(),
        root_stem,
        &meta("orchestrator", "root", Some(thread)),
        &[(1_000, 10, 0)],
    );
    write_transcript_turns(
        tmp.path(),
        &format!("{root_stem}__child_coder"),
        &meta("coding_agent", "subagent", Some("worker-xyz")),
        &[(5_000, 50, 0)],
    );

    let spend = thread_spend(tmp.path(), thread);
    let (coder, runs) = spend.subagents.get("coding_agent").expect("child counted");

    assert_eq!(spend.root.input_tokens, 1_000);
    assert_eq!(coder.input_tokens, 5_000);
    assert_eq!(*runs, 1);
    assert_eq!(spend.root.input_tokens + coder.input_tokens, 6_000);
}

/// A grandchild chains another `__`, and must be counted once at its own depth
/// rather than dropped or folded into its parent twice.
#[test]
fn counts_a_grandchild_delegation_once() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let thread = "thread-deep";
    let root_stem = "1790000002_orchestrator_deep";
    write_transcript_turns(
        tmp.path(),
        root_stem,
        &meta("orchestrator", "root", Some(thread)),
        &[(1_000, 10, 0)],
    );
    write_transcript_turns(
        tmp.path(),
        &format!("{root_stem}__child"),
        &meta("researcher", "subagent", Some("worker-1")),
        &[(2_000, 20, 0)],
    );
    write_transcript_turns(
        tmp.path(),
        &format!("{root_stem}__child__grandchild"),
        &meta("coding_agent", "subagent", Some("worker-2")),
        &[(4_000, 40, 0)],
    );

    let spend = thread_spend(tmp.path(), thread);
    let total: u64 = spend.root.input_tokens
        + spend
            .subagents
            .values()
            .map(|(child, _)| child.input_tokens)
            .sum::<u64>();
    assert_eq!(
        spend.subagents.len(),
        2,
        "child and grandchild both counted"
    );
    assert_eq!(total, 7_000);
}

/// Legacy transcripts already on disk carry zero `_meta` rollups. They must keep
/// working, which they do because the aggregate never reads the header.
#[test]
fn ignores_the_dead_meta_rollup_on_legacy_transcripts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let thread = "thread-legacy";
    let legacy = meta("orchestrator", "root", Some(thread));
    assert_eq!(
        legacy.input_tokens, 0,
        "fixture must reproduce the on-disk state: a zero header"
    );
    write_transcript_turns(
        tmp.path(),
        "1789983533_orchestrator_legacy",
        &legacy,
        &[(12_495, 23, 12_288)],
    );

    let spend = thread_spend(tmp.path(), thread);
    assert_eq!(
        spend.root.input_tokens, 12_495,
        "the per-turn record is authoritative, the zero header is not"
    );
    assert_eq!(spend.root.cached_input_tokens, 12_288);
}

/// A thread with no transcripts at all reports nothing, and a thread whose
/// transcripts recorded no spend must not claim usage: the UI replaces its live
/// bucket with this payload, so a false `has_usage` zeroes a turn in flight.
#[test]
fn reports_no_usage_for_an_unknown_or_spendless_thread() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let absent = thread_spend(tmp.path(), "thread-nope");
    assert!(!absent.found_transcript);
    assert_eq!(absent.root.input_tokens, 0);

    // A transcript that exists but recorded a single all-zero turn. The codec
    // omits an all-zero usage record, so this is the shape on disk: rows, no
    // usage.
    let thread = "thread-empty";
    let dir = tmp.path().join("session_raw");
    std::fs::create_dir_all(&dir).expect("create session_raw");
    let path = dir.join("1790000003_orchestrator_empty.jsonl");
    append_transcript_turn(
        &path,
        &[],
        &[TranscriptMessage::new("user", "hi")],
        &meta("orchestrator", "root", Some(thread)),
        None,
        Some("req-0"),
    )
    .expect("append turn");

    let spend = thread_spend(tmp.path(), thread);
    assert!(spend.found_transcript, "the file is there");
    assert_eq!(spend.root.turns, 0, "but it recorded no spend");
    assert_eq!(spend.root.input_tokens, 0);
}
