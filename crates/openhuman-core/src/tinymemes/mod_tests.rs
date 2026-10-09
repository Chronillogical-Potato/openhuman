use super::*;

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use tinymemes::decisions::{
    Answer, ChoiceAnswer, EvaluationRequest, EvaluationResponse, NoulAnswer, ScoreAnswer, Usage,
};
use tinymemes::{BoxError, ChatModel, Evaluator, MemeEngine, Region};

use crate::threads::store::ConversationMessage;

/// Answers every reading question the same way, whatever the chat.
struct ScriptedJev {
    frankness: f64,
    playful: f64,
    serious: f64,
    slang_enough: f64,
    meme: Option<&'static str>,
}

#[async_trait]
impl Evaluator for ScriptedJev {
    async fn evaluate(&self, request: &EvaluationRequest) -> Result<EvaluationResponse, BoxError> {
        let choice = |c: &str| {
            Answer::Choice(ChoiceAnswer {
                choice: c.to_owned(),
                probabilities: BTreeMap::new(),
                confidence: 0.9,
            })
        };
        let noul = |p: f64| Answer::Noul(NoulAnswer { noul: p });
        let mut answers = BTreeMap::from([
            ("chat_intent".to_owned(), choice("celebration")),
            ("reply_intent".to_owned(), choice("celebration")),
            (
                "frankness".to_owned(),
                Answer::Score(ScoreAnswer {
                    score: self.frankness * 4.0,
                    legend: BTreeMap::new(),
                    probabilities: BTreeMap::new(),
                    confidence: 0.8,
                }),
            ),
            ("playful".to_owned(), noul(self.playful)),
            ("serious".to_owned(), noul(self.serious)),
        ]);
        if request.questions.contains_key("slang_enough") {
            answers.insert("slang_enough".to_owned(), noul(self.slang_enough));
            answers.insert("slang_best".to_owned(), choice("yaar"));
        }
        if let (Some(meme), true) = (self.meme, request.questions.contains_key("meme_best")) {
            answers.insert("meme_best".to_owned(), choice(meme));
        }
        Ok(EvaluationResponse {
            model: "jev-latest".into(),
            answers,
            usage: Usage::default(),
        })
    }
}

struct FailingJev;

#[async_trait]
impl Evaluator for FailingJev {
    async fn evaluate(&self, _: &EvaluationRequest) -> Result<EvaluationResponse, BoxError> {
        Err("jev down".into())
    }
}

/// Returns a fixed rewrite, after an optional delay.
struct ScriptedModel {
    rewrite: &'static str,
    delay: Duration,
}

#[async_trait]
impl ChatModel for ScriptedModel {
    async fn complete(&self, _system: &str, _user: &str) -> Result<String, BoxError> {
        tokio::time::sleep(self.delay).await;
        Ok(self.rewrite.to_owned())
    }
}

fn frank(slang_enough: f64) -> Arc<ScriptedJev> {
    Arc::new(ScriptedJev {
        frankness: 0.95,
        playful: 0.95,
        serious: 0.0,
        slang_enough,
        meme: None,
    })
}

fn model(rewrite: &'static str) -> Arc<ScriptedModel> {
    Arc::new(ScriptedModel {
        rewrite,
        delay: Duration::ZERO,
    })
}

fn host(
    dir: &tempfile::TempDir,
    jev: Arc<dyn Evaluator>,
    chat: Arc<dyn ChatModel>,
    region: Option<Region>,
) -> Arc<host::Host> {
    let mut builder = MemeEngine::builder(jev, chat).learn_inline(None);
    if let Some(region) = region {
        builder = builder.region(region);
    }
    Arc::new(host::Host::new(
        builder.build(),
        dir.path().to_path_buf(),
        VecDeque::new(),
        0,
    ))
}

fn config_in(dir: &std::path::Path) -> Config {
    let mut config = Config::default();
    config.config_path = dir.join("config.toml");
    config.workspace_dir = dir.join("workspace");
    config.secrets.encrypt = false;
    config
}

fn turn(request_id: &str) -> TurnInfo<'_> {
    TurnInfo {
        request_id,
        bucket: 7,
        turn_ms: 1200,
    }
}

fn msg(id: &str, sender: &str, content: &str) -> ConversationMessage {
    ConversationMessage {
        id: id.to_owned(),
        content: content.to_owned(),
        message_type: "text".to_owned(),
        extra_metadata: serde_json::Value::Null,
        sender: sender.to_owned(),
        created_at: "2026-10-09T00:00:00Z".to_owned(),
    }
}

fn thread() -> Vec<ConversationMessage> {
    vec![
        msg("u1", "user", "yo the deploy finally went green lmaooo"),
        msg("r1", "agent", "Nice. Want me to tag the release?"),
    ]
}

const REPLY: &str = "Done. Tagged the release.";
const REWRITE: &str = "Done bhai, release tagged, full send 🎉";

/// Let spawned research tasks run to completion.
async fn settle() {
    for _ in 0..20 {
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[test]
fn only_real_answers_are_remixed() {
    let placeholder = "You've hit your inference budget.";
    assert!(should_remix("Here's the fix.", placeholder));
    assert!(!should_remix("", placeholder));
    assert!(!should_remix("   \n", placeholder));
    assert!(!should_remix(placeholder, placeholder));
}

#[test]
fn only_the_treatment_arm_passes_the_gate() {
    let t = turn("req-gate");
    assert!(!gate(Arm::Disabled, &t));
    assert!(!gate(Arm::Control, &t));
    assert!(gate(Arm::Treatment, &t));
}

#[test]
fn the_budget_defaults_to_twenty_seconds() {
    if std::env::var(TIMEOUT_ENV).is_err() {
        assert_eq!(budget(), DEFAULT_TIMEOUT);
    }
}

#[tokio::test]
async fn a_disabled_flag_leaves_the_task_reply_alone() {
    // The flag is unset in tests, so every thread is in the disabled arm.
    if std::env::var(arm::FLAG_ENV).is_ok() {
        return;
    }
    assert!(!holds_text_stream("thread-1"));
    let dir = tempfile::TempDir::new().unwrap();
    let config = config_in(dir.path());
    let mut reply = REPLY.to_owned();
    remix_task_reply(
        &config,
        "thread-1",
        "req-1",
        "tag it",
        &mut reply,
        "placeholder",
        Duration::from_millis(5),
    )
    .await;
    assert_eq!(reply, REPLY);

    let mut empty = String::new();
    remix_task_reply(
        &config,
        "thread-1",
        "req-1",
        "tag it",
        &mut empty,
        "placeholder",
        Duration::ZERO,
    )
    .await;
    assert!(empty.is_empty());
}

#[tokio::test]
async fn load_builds_the_engine_once_per_workspace() {
    let dir = tempfile::TempDir::new().unwrap();
    let config = config_in(dir.path());
    let (first, _messages) = load(&config, "thread-missing").await.expect("engine");
    let (second, _) = load(&config, "thread-missing").await.expect("engine");
    assert!(Arc::ptr_eq(&first, &second), "the engine was rebuilt");
    assert!(dir.path().join("workspace").join("tinymemes").is_dir());
}

#[tokio::test]
async fn a_frank_chat_is_remixed_and_remembered() {
    let dir = tempfile::TempDir::new().unwrap();
    let host = host(&dir, frank(0.9), model(REWRITE), Some(Region::global()));
    let out = remix_with(
        &host,
        &thread(),
        &turn("req-remix"),
        Instant::now(),
        "ya send it",
        REPLY,
        Duration::from_secs(5),
    )
    .await
    .expect("remixed");
    assert_ne!(out.trim(), REPLY);
    // The delivered message is remembered as remixed, and state is saved.
    let id = crate::threads::store::run_reply_message_id("req-remix");
    assert!(host.is_remixed(&id));
    let remixed: Vec<String> =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("remixed.json")).unwrap())
            .unwrap();
    assert_eq!(remixed, [id]);
    assert!(dir.path().join("slang-index.json").exists());
    assert!(dir.path().join("meme-index.json").exists());
}

#[tokio::test]
async fn a_short_slang_index_researches_after_delivery() {
    let dir = tempfile::TempDir::new().unwrap();
    // Jev judges the index short of slang; with no researcher configured the
    // background task finds nothing to do and the reply still goes out.
    let host = host(&dir, frank(0.05), model(REWRITE), Some(Region::global()));
    let out = remix_with(
        &host,
        &thread(),
        &turn("req-slang"),
        Instant::now(),
        "ya send it",
        REPLY,
        Duration::from_secs(5),
    )
    .await;
    assert!(out.is_some());
    settle().await;
    assert!(dir.path().join("slang-index.json").exists());
}

#[tokio::test]
async fn no_fitting_meme_starts_meme_research() {
    let dir = tempfile::TempDir::new().unwrap();
    let jev = Arc::new(ScriptedJev {
        meme: Some(tinymemes::reading::NONE_FIT),
        ..*frank(0.9)
    });
    // The default (India) region offers catalog memes, so Jev is asked to
    // pick one and answers that none fit.
    let host = host(&dir, jev, model(REWRITE), None);
    let _ = remix_with(
        &host,
        &thread(),
        &turn("req-meme"),
        Instant::now(),
        "ya send it",
        REPLY,
        Duration::from_secs(5),
    )
    .await;
    settle().await;
    assert!(dir.path().join("meme-index.json").exists());
}

#[tokio::test]
async fn an_off_rated_chat_is_left_unchanged() {
    let dir = tempfile::TempDir::new().unwrap();
    let jev = Arc::new(ScriptedJev {
        frankness: 0.0,
        playful: 0.0,
        serious: 0.9,
        slang_enough: 0.9,
        meme: None,
    });
    let host = host(&dir, jev, model(REWRITE), Some(Region::global()));
    let out = remix_with(
        &host,
        &thread(),
        &turn("req-off"),
        Instant::now(),
        "ya send it",
        REPLY,
        Duration::from_secs(5),
    )
    .await;
    assert_eq!(out, None);
    assert!(!dir.path().join("remixed.json").exists());
}

#[tokio::test]
async fn a_jev_failure_sends_the_original() {
    let dir = tempfile::TempDir::new().unwrap();
    let host = host(&dir, Arc::new(FailingJev), model(REWRITE), None);
    let out = remix_with(
        &host,
        &thread(),
        &turn("req-error"),
        Instant::now(),
        "ya send it",
        REPLY,
        Duration::from_secs(5),
    )
    .await;
    assert_eq!(out, None);
}

#[tokio::test]
async fn a_slow_remix_times_out_to_the_original() {
    let dir = tempfile::TempDir::new().unwrap();
    let slow = Arc::new(ScriptedModel {
        rewrite: REWRITE,
        delay: Duration::from_secs(5),
    });
    let host = host(&dir, frank(0.9), slow, Some(Region::global()));
    let out = remix_with(
        &host,
        &thread(),
        &turn("req-timeout"),
        Instant::now(),
        "ya send it",
        REPLY,
        Duration::from_millis(50),
    )
    .await;
    assert_eq!(out, None);
}

#[test]
fn the_log_line_handles_a_missing_outcome() {
    // Exercised for its formatting paths; it must not panic without a reading.
    log_outcome(&turn("req-log"), Instant::now(), "engine_unavailable", None);
}

#[tokio::test]
async fn the_turn_line_reports_the_outcome_without_user_content() {
    let dir = tempfile::TempDir::new().unwrap();
    let host = host(&dir, frank(0.9), model(REWRITE), Some(Region::global()));
    let turns = host::history_turns(&thread(), "ya send it", |_| false);
    let outcome = host.engine.process(&turns, REPLY).await;
    let line = turn_line(&turn("req-line"), 42, "remixed", Some(&outcome));
    assert!(line.starts_with("[tinymemes] turn arm=treatment bucket=7 request_id=req-line"));
    assert!(line.contains("turn_ms=1200 remix_ms=42 outcome=remixed"));
    assert!(line.contains("tier=unhinged"), "{line}");
    assert!(!line.contains("send it") && !line.contains("Tagged"));

    let bare = turn_line(&turn("req-none"), 0, "timeout", None);
    assert!(bare.contains("score=-1 tier=none mode=none memes=0 rewrite_kept=true dupes=0"));
    assert!(bare.contains("meme_pick=none meme_p=none matches=none"));
}

#[tokio::test]
async fn every_arm_fails_open_without_working_providers() {
    let dir = tempfile::TempDir::new().unwrap();
    let config = config_in(dir.path());
    for arm in [Arm::Disabled, Arm::Control, Arm::Treatment] {
        // Treatment builds the real engine; with no provider configured every
        // model call fails, so the original reply is delivered.
        let out = remix_final_reply(
            arm,
            &config,
            "thread-real",
            "req-real",
            "ya send it",
            REPLY,
            Duration::from_millis(5),
        )
        .await;
        assert_eq!(out, None, "{arm:?}");
    }
}
