//! What the host keeps from a browser task once it stops: a content-free
//! summary in the log when the task stopped short (failed, or needs a person
//! or a plan), and — with tracing on — the module's full report under
//! `<workspace>/state/computer/tasks/`.
//!
//! A report holds page text (step notes, records and, when traced, every Jev
//! exchange), so it is written only to the workspace and never logged: the
//! log gets counts, the failed step's position and kind, and the module's own
//! one-line reason.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tinycomputer_bus::agent::{RescueOutcome, TaskReport, TaskStatus, TaskView};
use tinycomputer_bus::flow::StepOutcome;

use crate::config::Config;

#[cfg(test)]
#[path = "browser_task_report_tests.rs"]
mod tests;

/// How many stopped tasks are remembered, so a view followed again does not
/// record the same stop twice.
const REMEMBERED: usize = 64;
/// The most characters of a failure reason written to the log.
const REASON_CHARS: usize = 240;

/// Records `view` once per task and state: a log summary when it stopped
/// short, and the full report on disk when tracing is on and it settled.
/// Never fails the caller; a report that cannot be fetched is only logged.
pub(crate) async fn record(config: &Config, view: &TaskView) {
    let traced = super::computer_config::tracing_enabled(config);
    if !worth_recording(&view.status, traced) || !first_time(view) {
        return;
    }
    let report = match super::browser_task::report_with(config, view.id.clone(), traced).await {
        Ok(report) => report,
        Err(error) => {
            tracing::warn!(task = %view.id, %error, "[browser-task] report unavailable");
            return;
        }
    };
    if stopped_short(&view.status) {
        let summary = Summary::of(&view.status, &report);
        tracing::warn!(
            task = %view.id,
            state = status_name(&view.status),
            step = ?summary.failed_step,
            kind = summary.failed_kind.unwrap_or("-"),
            steps = summary.steps,
            jev_calls = summary.jev_calls,
            actions = summary.actions,
            rescues = summary.rescues,
            recovered = summary.recovered,
            reason = %reason(&view.status),
            "[browser-task] task stopped short"
        );
    }
    if traced {
        write(config, &report).await;
    }
}

/// A task that ended without doing what it was asked.
fn stopped_short(status: &TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Failed { .. } | TaskStatus::NeedsHuman { .. } | TaskStatus::NeedsPlan { .. }
    )
}

fn worth_recording(status: &TaskStatus, traced: bool) -> bool {
    stopped_short(status) || (traced && !matches!(status, TaskStatus::Running))
}

/// Whether this task has not been recorded in this state yet.
fn first_time(view: &TaskView) -> bool {
    static SEEN: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
    let key = format!(
        "{}|{}|{:.3}",
        view.id,
        status_name(&view.status),
        view.progress
    );
    let mut seen = SEEN
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if seen.contains(&key) {
        return false;
    }
    if seen.len() == REMEMBERED {
        seen.pop_front();
    }
    seen.push_back(key);
    true
}

fn status_name(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Running => "running",
        TaskStatus::NeedsInput { .. } => "needs_input",
        TaskStatus::NeedsApproval { .. } => "needs_approval",
        TaskStatus::Checkpoint { .. } => "checkpoint",
        TaskStatus::NeedsHuman { .. } => "needs_human",
        TaskStatus::NeedsPlan { .. } => "needs_plan",
        TaskStatus::Done { .. } => "done",
        TaskStatus::Failed { .. } => "failed",
        TaskStatus::Cancelled => "cancelled",
    }
}

/// The module's own reason for stopping, cut to [`REASON_CHARS`].
fn reason(status: &TaskStatus) -> String {
    let reason = match status {
        TaskStatus::Failed { reason, .. } | TaskStatus::NeedsHuman { reason, .. } => {
            reason.as_str()
        }
        TaskStatus::NeedsPlan { .. } => "no planner is configured",
        _ => "",
    };
    reason.chars().take(REASON_CHARS).collect()
}

/// The counts a stopped task is logged with; nothing a page showed.
#[derive(Debug, PartialEq, Eq)]
struct Summary {
    steps: usize,
    jev_calls: u64,
    actions: usize,
    rescues: usize,
    recovered: usize,
    failed_step: Option<usize>,
    failed_kind: Option<&'static str>,
}

impl Summary {
    fn of(status: &TaskStatus, report: &TaskReport) -> Self {
        let failed_step = match status {
            TaskStatus::Failed { step, .. } => *step,
            _ => None,
        };
        Self {
            steps: report.steps.len(),
            jev_calls: report
                .steps
                .iter()
                .map(|step| u64::from(step.jev_calls))
                .sum(),
            actions: report.steps.iter().map(|step| step.actions.len()).sum(),
            rescues: report.rescues.len(),
            recovered: report
                .rescues
                .iter()
                .filter(|rescue| rescue.outcome == RescueOutcome::Recovered)
                .count(),
            failed_step,
            failed_kind: report
                .steps
                .iter()
                .rev()
                .find(|step| step.outcome == StepOutcome::Failed)
                .map(|step| kind_name(&step.kind)),
        }
    }
}

/// A step kind as the log may show it: one of the flow grammar's own words,
/// never free text.
fn kind_name(kind: &str) -> &'static str {
    const KINDS: &[&str] = &[
        "open",
        "browse",
        "do",
        "enter",
        "choose",
        "read",
        "extract",
        "pick",
        "verify",
        "wait_for",
        "if",
        "repeat_until",
        "stop_before",
    ];
    KINDS
        .iter()
        .find(|known| kind.eq_ignore_ascii_case(known))
        .copied()
        .unwrap_or("other")
}

/// Where a report of task `id`, recorded at `millis`, is written.
fn report_path(config: &Config, id: &str, millis: u128) -> PathBuf {
    let id: String = id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .collect();
    super::computer_config::trace_dir(config)
        .join("tasks")
        .join(format!("{millis}-{id}.json"))
}

async fn write(config: &Config, report: &TaskReport) {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    let path = report_path(config, &report.view.id.0, millis);
    let bytes = match serde_json::to_vec_pretty(report) {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(%error, "[browser-task] report not serializable");
            return;
        }
    };
    if let Some(dir) = path.parent() {
        if let Err(error) = tokio::fs::create_dir_all(dir).await {
            tracing::warn!(%error, "[browser-task] report directory unavailable");
            return;
        }
    }
    match tokio::fs::write(&path, &bytes).await {
        Ok(()) => tracing::info!(
            task = %report.view.id,
            path = %path.display(),
            bytes = bytes.len(),
            exchanges = report.trace.len(),
            "[browser-task] report written"
        ),
        Err(error) => tracing::warn!(%error, "[browser-task] report not written"),
    }
}
