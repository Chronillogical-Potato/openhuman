use super::*;

use chrono::Utc;
use tinyagents_harness::summarization::ConcatSummarizer;

use crate::memory::lifecycle::hooks::{post_turn, pre_turn, PostTurnInput, PreTurnInput};
use crate::memory::scope::MemoryIdentity;
use crate::memory::test_fixtures::{bind_reference, config_in};

struct Failing;

#[async_trait]
impl Summarizer for Failing {
    async fn summarize(&self, _messages: &[Message]) -> Result<SummaryRecord> {
        Err(tinyagents_harness::error::TinyAgentsError::Validation(
            "summarizer down".into(),
        ))
    }
}

async fn bound_turn(tmp: &tempfile::TempDir) -> Arc<MemoryTurn> {
    let config = config_in(tmp);
    bind_reference(&config);
    let identity = MemoryIdentity::agent("orchestrator").resolve(&config);
    pre_turn(
        &config,
        &identity,
        PreTurnInput {
            thread_id: "t".into(),
            turn_index: 0,
            user_text: "our launch date is March 3rd".into(),
            in_prompt_from: 0,
            at: Utc::now(),
            resumed_after_compaction: false,
        },
    )
    .await;
    post_turn(
        &config,
        &identity,
        PostTurnInput {
            thread_id: "t".into(),
            turn_index: 1,
            assistant_text: "Noted: launch on March 3rd.".into(),
            tool_calls: Vec::new(),
            at: Utc::now(),
        },
    )
    .await;
    Arc::new(MemoryTurn {
        config: Arc::new(config),
        identity,
        thread_id: "t".into(),
        pack: None,
    })
}

fn dropped() -> Vec<Message> {
    vec![
        Message::system("ignored"),
        Message::user("our launch date is March 3rd"),
        Message::assistant("Noted: launch on March 3rd."),
        Message::tool("c1", "tool output is the summarizer's"),
    ]
}

#[test]
fn only_user_and_assistant_text_become_turns() {
    let turns = dropped_turns(&dropped());
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0].role, Role::User);
    assert_eq!(turns[1].role, Role::Assistant);
}

#[tokio::test]
async fn the_checkpoint_carries_what_memory_recalls() {
    let tmp = tempfile::tempdir().unwrap();
    let turn = bound_turn(&tmp).await;
    let summarizer = MemoryRecallSummarizer::wrap(Box::new(ConcatSummarizer), Some(turn));
    let record = summarizer.summarize(&dropped()).await.unwrap();
    let text = record.summary.text();
    assert!(text.contains(RECALLED_HEADING), "{text}");
    assert!(text.contains("March 3rd"));

    let request = SummaryRequest::new(dropped());
    let record = summarizer.summarize_request(&request).await.unwrap();
    assert!(record.summary.text().contains(RECALLED_HEADING));
}

#[tokio::test]
async fn without_recall_the_summarizer_is_unchanged_and_its_failure_is_its_own() {
    let tmp = tempfile::tempdir().unwrap();
    let turn = bound_turn(&tmp).await;

    let plain = MemoryRecallSummarizer::wrap(Box::new(ConcatSummarizer), None);
    let text = plain.summarize(&dropped()).await.unwrap().summary.text();
    assert!(!text.contains(RECALLED_HEADING));

    let mut quiet = (*turn).clone();
    quiet.identity.recall = false;
    let off = MemoryRecallSummarizer::wrap(Box::new(ConcatSummarizer), Some(Arc::new(quiet)));
    assert!(!off
        .summarize(&dropped())
        .await
        .unwrap()
        .summary
        .text()
        .contains(RECALLED_HEADING));

    let failing = MemoryRecallSummarizer::wrap(Box::new(Failing), Some(turn));
    assert!(failing.summarize(&dropped()).await.is_err());
}

#[test]
fn an_earlier_recall_section_is_dropped_before_the_new_one() {
    let mut summary = Message::user(format!(
        "the summary so far\n\n{RECALLED_HEADING}\n\n- an old recall"
    ));
    strip_recalled(&mut summary);
    assert_eq!(summary.text(), "the summary so far");

    let mut untouched = Message::user("nothing recalled here");
    strip_recalled(&mut untouched);
    assert_eq!(untouched.text(), "nothing recalled here");
}

#[tokio::test]
async fn a_second_compaction_keeps_one_recall_section() {
    let tmp = tempfile::tempdir().unwrap();
    let turn = bound_turn(&tmp).await;
    let summarizer = MemoryRecallSummarizer::wrap(Box::new(ConcatSummarizer), Some(turn));
    let first = summarizer.summarize(&dropped()).await.unwrap();
    let mut again = dropped();
    again.insert(0, first.summary.clone());
    let second = summarizer.summarize(&again).await.unwrap();
    assert_eq!(second.summary.text().matches(RECALLED_HEADING).count(), 1);
}
