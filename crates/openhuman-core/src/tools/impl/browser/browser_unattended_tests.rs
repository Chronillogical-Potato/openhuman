use super::super::pending::Pending;
use super::super::{approve_browser_action, task_actions::approve_task_action};
use super::*;
use crate::modules::browser::BrowserClient;
use std::sync::Arc;
use tinycomputer_bus::agent::TaskId;
use tinycomputer_bus::browser::{Action, SessionId, Target};

fn listing(actions: &[&str]) -> BrowserConfig {
    BrowserConfig {
        unattended_actions: actions.iter().map(|action| (*action).to_owned()).collect(),
        ..BrowserConfig::default()
    }
}

fn automation(source: TrustedAutomationSource) -> AgentTurnOrigin {
    AgentTurnOrigin::TrustedAutomation {
        job_id: "job-1".into(),
        source,
    }
}

fn cron() -> AgentTurnOrigin {
    automation(TrustedAutomationSource::Cron)
}

fn web_chat() -> AgentTurnOrigin {
    AgentTurnOrigin::WebChat {
        thread_id: "thread-1".into(),
        client_id: "client-1".into(),
        request_id: None,
    }
}

fn external_channel() -> AgentTurnOrigin {
    AgentTurnOrigin::ExternalChannel {
        channel: "telegram".into(),
        sender: Some("someone".into()),
        reply_target: "chat-1".into(),
        message_id: "m-1".into(),
    }
}

#[test]
fn trusted_unattended_sources_may_take_a_listed_action() {
    let browser = listing(&["click"]);
    for origin in [
        cron(),
        automation(TrustedAutomationSource::Background),
        automation(TrustedAutomationSource::Workflow {
            require_approval: false,
        }),
    ] {
        assert!(allowed_for(Some(&origin), &browser, "click"), "{origin:?}");
    }
}

#[test]
fn cron_may_not_take_an_action_that_is_not_listed() {
    assert!(!allowed_for(Some(&cron()), &listing(&["click"]), "fill"));
}

#[test]
fn an_empty_list_allows_nothing_for_any_origin() {
    let browser = BrowserConfig::default();
    for origin in [cron(), web_chat(), external_channel()] {
        assert!(!allowed_for(Some(&origin), &browser, "click"), "{origin:?}");
    }
}

#[test]
fn interactive_remote_and_unlabelled_turns_never_skip_the_gate() {
    let browser = listing(&["click", "task_step"]);
    for origin in [
        web_chat(),
        external_channel(),
        automation(TrustedAutomationSource::Workflow {
            require_approval: true,
        }),
        AgentTurnOrigin::Cli,
        AgentTurnOrigin::DirectChat,
        AgentTurnOrigin::Unknown,
    ] {
        assert!(!allowed_for(Some(&origin), &browser, "click"), "{origin:?}");
    }
    assert!(!allowed_for(None, &browser, "click"));
}

#[tokio::test]
async fn the_current_turn_origin_decides() {
    let browser = listing(&["press"]);
    assert!(turn_origin::with_origin(cron(), async { allow_current(&browser, "press", "abc") }).await);
    assert!(!turn_origin::with_origin(web_chat(), async { allow_current(&browser, "press", "abc") }).await);
    assert!(!allow_current(&browser, "press", "abc"));
}

fn pending() -> Pending {
    Pending {
        task: TaskId::new("t-1"),
        action: "Open the next page".into(),
        target: "Next".into(),
        token: "token".into(),
    }
}

#[tokio::test]
async fn a_listed_task_step_is_approved_for_cron_without_a_gate() {
    let approved = turn_origin::with_origin(
        cron(),
        approve_task_action(&pending(), &listing(&["task_step"])),
    )
    .await;
    assert!(approved.unwrap());
}

#[tokio::test]
async fn an_unlisted_or_untrusted_task_step_still_needs_the_gate() {
    // With no interactive gate this errors; with one, the forced gate denies a
    // non-WebChat turn. Either way the step is not approved.
    let unlisted =
        turn_origin::with_origin(cron(), approve_task_action(&pending(), &listing(&["click"])))
            .await;
    assert!(!matches!(unlisted, Ok(true)));
    let remote = turn_origin::with_origin(
        external_channel(),
        approve_task_action(&pending(), &listing(&["task_step"])),
    )
    .await;
    assert!(!matches!(remote, Ok(true)));
    let workflow = turn_origin::with_origin(
        automation(TrustedAutomationSource::Workflow {
            require_approval: true,
        }),
        approve_task_action(&pending(), &listing(&["task_step"])),
    )
    .await;
    assert!(!matches!(workflow, Ok(true)));
}

fn offline_client(actions: &[&str]) -> BrowserClient {
    let mut config = crate::config::Config::default();
    // No module and no download: a call that reached the module fails fast.
    config.modules.enabled = false;
    config.modules.allow_download = false;
    config.browser = listing(actions);
    BrowserClient::new(Arc::new(config))
}

fn click() -> Action {
    Action::Click {
        target: Target::reference("e1"),
        new_tab: false,
    }
}

#[tokio::test]
async fn a_listed_direct_action_runs_for_cron_without_reading_the_page() {
    let client = offline_client(&["click"]);
    let session = SessionId::new("s-1");
    let outcome =
        turn_origin::with_origin(cron(), approve_browser_action(&client, &session, &click(), false))
            .await;
    assert!(outcome.is_ok(), "{outcome:?}");
}

#[tokio::test]
async fn an_unlisted_or_untrusted_direct_action_is_not_waved_through() {
    let session = SessionId::new("s-1");
    let unlisted = offline_client(&["press"]);
    let outcome = turn_origin::with_origin(
        cron(),
        approve_browser_action(&unlisted, &session, &click(), false),
    )
    .await;
    assert!(outcome.is_err());
    let listed = offline_client(&["click"]);
    let outcome = turn_origin::with_origin(
        external_channel(),
        approve_browser_action(&listed, &session, &click(), false),
    )
    .await;
    assert!(outcome.is_err());
    let outcome = approve_browser_action(&listed, &session, &click(), false).await;
    assert!(outcome.is_err());
}

#[derive(Clone, Default)]
struct Logs(Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for Logs {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn an_allowed_action_is_logged_by_kind_and_digest_without_its_input() {
    let logs = Logs::default();
    let writer = logs.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    // A sibling test may have registered the callsite before this subscriber
    // existed; recompute its interest so this thread sees the event.
    tracing::callsite::rebuild_interest_cache();
    let client = offline_client(&["fill"]);
    let fill = Action::Fill {
        target: Target::selector("#secret-field"),
        value: "hunter2-password".into(),
    };
    let session = SessionId::new("s-1");
    turn_origin::with_origin(cron(), approve_browser_action(&client, &session, &fill, false))
        .await
        .unwrap();
    let text = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    let line = text
        .lines()
        .find(|line| line.contains("[browser] unattended action allowed"))
        .unwrap_or_else(|| panic!("no unattended log line in {text:?}"));
    assert!(line.contains("action=\"fill\"") || line.contains("action=fill"), "{line}");
    assert!(line.contains("action_digest="), "{line}");
    assert!(line.contains("TrustedAutomation(Cron)"), "{line}");
    assert!(!line.contains("hunter2-password") && !line.contains("#secret-field"), "{line}");
    assert!(!line.contains("job-1"), "{line}");
}
