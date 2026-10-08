//! [`RepeatedToolFailureMiddleware`]: halt (or nudge) the run when tool calls
//! keep failing with no progress, driving the crate no-progress ladder plus
//! OpenHuman's recoverable-failure headroom and terminal-inference fast-halt.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use tinyagents_harness::context::RunContext;
use tinyagents_harness::error::Result as TaResult;
use tinyagents_harness::middleware::{repeat_guard_marker, Middleware, ToolInvocationIdentity};
use tinyagents_harness::no_progress::{
    ClassifiedFailure, ClassifiedFailureTracker, NoProgress, NoProgressTracker, ToolAttempt,
};
use tinyagents_harness::steering::{SteeringCommand, SteeringHandle};
use tinyinference_llm::tool::ToolCall as TaToolCall;
use tinytools::ToolResult as TaToolResult;

use super::fetched_site::{fetch_host_scope, fetched_site_policy, heuristic_text};
use super::loop_guards::{
    is_repeat_call_exempt, RECOVERABLE_NO_PROGRESS_FAILURE_THRESHOLD,
    RECOVERABLE_REPEAT_FAILURE_THRESHOLD,
};
use super::nudge_injector::PendingNudgeInjector;
pub(crate) use crate::inference::failure_copy::user_actionable_escalation;
use crate::inference::failure_copy::{
    recoverable_identical_halt_summary, recoverable_no_progress_halt_summary,
    terminal_inference_failure_kind, terminal_inference_halt_summary,
};
use tinyinference_llm::failure::is_recoverable_failure_text as is_recoverable_tool_failure;

/// `after_tool`: stop (or nudge) the run when tool calls keep failing with no
/// progress (issue #4249). The legacy tool loop's progress guard surfaced a
/// root-cause halt summary — a security/approval denial re-issued unchanged, an
/// identical error retried, or *different* commands all failing — instead of
/// burning the whole iteration budget and ending on a generic cap error. The
/// tinyagents path kept only the model/tool call caps, so this reinstates the
/// guard as a graph middleware.
///
/// As of tinyagents 1.5.0 the escalation ladder itself lives in the crate
/// ([`NoProgressTracker`], extracted upstream from OpenHuman #4389). This
/// middleware is now a **thin driver**: it captures the per-call argument
/// fingerprint (the tool result carries no arguments), feeds each outcome into
/// [`NoProgressTracker::record`], and lowers the returned [`NoProgress`] verdict
/// into OpenHuman steering. It owns only the OpenHuman-side policy:
///
/// - [`NoProgress::Continue`] — do nothing.
/// - [`NoProgress::Nudge`] — queue the crate's structured "no progress since
///   step X" corrective for the **next model request only** (see
///   [`PendingNudgeInjector`]) so the model changes strategy *before* the
///   same-strategy retry cap trips.
/// - [`NoProgress::Halt`] — record the crate's root-cause summary into the shared
///   [`HaltSummarySlot`](crate::agent::tinyagents::HaltSummarySlot) (the turn overrides its final
///   text with it) and pause the run via the shared steering handle (same
///   mechanism as the stop-hook / cap pausers), then [`reset`](NoProgressTracker::reset)
///   so a resumed run does not immediately re-pause on the latched state.
pub(crate) struct RepeatedToolFailureMiddleware {
    handle: SteeringHandle,
    halt_summary: crate::agent::tinyagents::HaltSummarySlot,
    /// Crate no-progress escalation ladder — the single source of the
    /// identical-failure / varied-failure / hard-reject logic (tinyagents 1.5.0).
    tracker: NoProgressTracker,
    classified: ClassifiedFailureTracker,
    /// Monotonic tool-outcome counter, used only for the crate's "no progress
    /// since step X" nudge wording. Not the model-call count, but a stable,
    /// increasing marker is all the wording needs.
    step: AtomicUsize,
    /// call_id → argument fingerprint, captured in `before_tool` (the tool result
    /// carries no arguments). Folded into the identical-repeat signature so the
    /// "identical arguments" halt only trips on the *same* args — two different
    /// argument sets that happen to share a first error line don't count as a
    /// repeat and can't pre-empt the generic no-progress backstop.
    arg_sigs: std::sync::Mutex<std::collections::HashMap<String, String>>,
    /// Call ID to stable target identity. Query strings and free-form prompts
    /// are excluded so varying a query cannot evade a resource-level blocker.
    target_scopes: std::sync::Mutex<std::collections::HashMap<String, String>>,
    /// Recoverable-failure ladder (issue #4463): transient failures (timeouts,
    /// connection resets, rate limits, 5xx) are routed here instead of the crate
    /// tracker so they get the legacy extended headroom
    /// ([`RECOVERABLE_REPEAT_FAILURE_THRESHOLD`] identical /
    /// [`RECOVERABLE_NO_PROGRESS_FAILURE_THRESHOLD`] consecutive) rather than the
    /// crate's fixed 3/6, which is right only for deterministic failures.
    /// `tool\u{1f}args` → identical-failure count; persists across the turn.
    recoverable_sig_counts: std::sync::Mutex<std::collections::HashMap<String, u32>>,
    /// Consecutive recoverable-looking failures with no success in between. Reset
    /// on any success or non-recoverable failure (mirrors the legacy guard).
    recoverable_consecutive: AtomicU32,
    /// Corrective nudges for the next model request, drained by
    /// [`PendingNudgeInjector`]. Never sent through steering: an injected
    /// message joins the working transcript and is committed into durable
    /// history, where it replays as a stale instruction on every later turn
    /// (#6725).
    pending_nudges: Arc<Mutex<Vec<String>>>,
    /// call_id → a one-line rendering of the call (a shell command verbatim,
    /// any other tool's arguments as compact JSON), captured in `before_tool`
    /// so a halt can say what was tried and not only how the last attempt
    /// ended.
    recent_calls: std::sync::Mutex<std::collections::HashMap<String, String>>,
    /// The consecutive failures the ladder is counting, oldest first, each as
    /// its rendered call and first error line. Any success clears it. A halt
    /// summary carrying only the last error left the failing commands
    /// unrecoverable: one run was stopped after six different submissions and
    /// kept no record of what any of them sent.
    recent_failures: std::sync::Mutex<std::collections::VecDeque<String>>,
    /// `tool\u{1f}args` of the last finished command that exited non-zero,
    /// kept so a *repeat* of it still counts as a failure while a different
    /// command's non-zero exit counts as information (see `after_tool`).
    last_exit_report: std::sync::Mutex<Option<String>>,
}

impl RepeatedToolFailureMiddleware {
    /// `summary` with the calls the ladder counted appended (see
    /// [`with_failing_calls`]).
    fn halt_with(&self, summary: String) -> String {
        let failures = self
            .recent_failures
            .lock()
            .map(|failures| failures.clone())
            .unwrap_or_default();
        // The crate's summary quotes the last error as the tool returned it;
        // the whole text is read by the model and persisted with the session,
        // so it is scrubbed here, at the one place every halt passes through.
        let summary = crate::security::scrub::sanitize_text(&summary).value;
        with_failing_calls(summary, &failures)
    }

    /// Build the breaker. `identical_threshold` (the identical-signature retry
    /// ceiling) is handed straight to [`NoProgressTracker::new`], which clamps it
    /// so a nudge always precedes a halt (a single failure is never a loop).
    pub(crate) fn new(
        handle: SteeringHandle,
        identical_threshold: usize,
        halt_summary: crate::agent::tinyagents::HaltSummarySlot,
    ) -> Self {
        Self {
            handle,
            halt_summary,
            tracker: NoProgressTracker::new(identical_threshold),
            classified: ClassifiedFailureTracker::default(),
            recent_calls: std::sync::Mutex::default(),
            recent_failures: std::sync::Mutex::default(),
            last_exit_report: std::sync::Mutex::default(),
            step: AtomicUsize::new(0),
            arg_sigs: std::sync::Mutex::new(std::collections::HashMap::new()),
            target_scopes: std::sync::Mutex::new(std::collections::HashMap::new()),
            recoverable_sig_counts: std::sync::Mutex::new(std::collections::HashMap::new()),
            recoverable_consecutive: AtomicU32::new(0),
            pending_nudges: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// The request-scoped half of this breaker. Register it **last**: its
    /// `before_model` must run after the transcript snapshot (which a failed
    /// turn persists) and after every reduction step.
    pub(crate) fn nudge_injector(&self) -> PendingNudgeInjector {
        PendingNudgeInjector {
            pending: self.pending_nudges.clone(),
        }
    }

    fn queue_nudge(&self, instruction: impl Into<String>) {
        if let Ok(mut pending) = self.pending_nudges.lock() {
            pending.push(instruction.into());
        }
    }

    /// Drain the queued nudges (what the injector does before a request).
    #[cfg(test)]
    pub(crate) fn take_pending_nudges(&self) -> Vec<String> {
        self.pending_nudges
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default()
    }

    /// Clear the consecutive recoverable-failure streak. Called on any success or
    /// non-recoverable failure (the per-signature identical counts persist across
    /// the turn, matching the legacy guard). Idempotent.
    fn reset_recoverable_streak(&self) {
        self.recoverable_consecutive.store(0, Ordering::SeqCst);
    }

    /// Record one recoverable failure and return a root-cause halt summary once
    /// its extended headroom is exhausted (identical `>=` [`RECOVERABLE_REPEAT_FAILURE_THRESHOLD`]
    /// or consecutive `>=` [`RECOVERABLE_NO_PROGRESS_FAILURE_THRESHOLD`]).
    fn record_recoverable(&self, tool: &str, arg_fp: &str, failure_text: &str) -> Option<String> {
        let key = format!("{tool}\u{1f}{arg_fp}");
        let count = self
            .recoverable_sig_counts
            .lock()
            .ok()
            .map(|mut counts| {
                let c = counts.entry(key).or_insert(0);
                *c += 1;
                *c
            })
            .unwrap_or(0);
        let consecutive = self.recoverable_consecutive.fetch_add(1, Ordering::SeqCst) + 1;
        tracing::debug!(
            tool,
            count,
            consecutive,
            "[tinyagents::mw] recoverable tool failure recorded with extended circuit-breaker headroom"
        );
        if count >= RECOVERABLE_REPEAT_FAILURE_THRESHOLD {
            return Some(recoverable_identical_halt_summary(
                tool,
                count,
                failure_text,
            ));
        }
        if consecutive >= RECOVERABLE_NO_PROGRESS_FAILURE_THRESHOLD {
            return Some(recoverable_no_progress_halt_summary(
                consecutive,
                tool,
                failure_text,
            ));
        }
        None
    }
}

/// Longest rendering of one call kept for a halt summary.
const RENDERED_CALL_CHARS: usize = 200;
/// How many consecutive failures a halt summary lists.
const RENDERED_FAILURES: usize = 8;

/// One line naming what a call did, safe for a halt summary the model reads
/// and the session persists: a `command` argument verbatim (the shell tools),
/// anything else as compact JSON. Secrets are redacted the way the approval
/// card redacts them, every URL loses its query and fragment (a fetch is
/// identified by host and path; a query can carry a token or a user's words),
/// API-key shapes in free text are scrubbed, and the result is
/// whitespace-collapsed and bounded so a heredoc or payload cannot swamp it.
fn render_call(tool: &str, arguments: &serde_json::Value) -> String {
    let raw = match arguments.get("command").and_then(serde_json::Value::as_str) {
        Some(command) => redact_command_credentials(command),
        None if arguments.is_null() => tool.to_owned(),
        None => strip_url_queries(&crate::security::approval::redact_args(arguments)).to_string(),
    };
    let without_queries = cut_url_queries(&raw);
    let scrubbed = crate::security::scrub::sanitize_text(&without_queries).value;
    crate::util::truncate_with_ellipsis(&scrubbed, RENDERED_CALL_CHARS).to_string()
}

/// A shell command with the values that follow credential-bearing flags,
/// headers and assignments replaced: `--password=…`, `--token …`,
/// `-H 'Authorization: Bearer …'`, `x-api-key: …`, `API_SECRET=…`. The flag,
/// header name or variable name stays, so the summary still says what the
/// command did; the value does not travel into the session. This is the
/// command-side counterpart of `redact_args` for the other tools; the
/// general scrubber still runs over the result for key shapes it knows.
fn redact_command_credentials(command: &str) -> String {
    use std::sync::LazyLock;
    static HEADER: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r#"(?i)((?:authorization|x-api-key|api-key|x-auth-token|x-access-token|cookie|proxy-authorization)\s*:\s*)([^'\"\s][^'\"]*)"#,
        )
        .expect("static regex")
    });
    static FLAG: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r#"(?i)(--?(?:password|passwd|pass|pwd|token|api[-_]?key|apikey|secret|auth|bearer|access[-_]?key|private[-_]?key|client[-_]?secret)(?:=|\s+))(['\"]?)([^\s'\"]+)"#,
        )
        .expect("static regex")
    });
    static ASSIGN: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r#"(?i)\b([A-Z0-9_]*(?:KEY|TOKEN|SECRET|PASSWORD|PASSWD|CREDENTIAL)[A-Z0-9_]*=)(['\"]?)([^\s'\"]+)"#,
        )
        .expect("static regex")
    });
    let out = HEADER.replace_all(command, "${1}[REDACTED]");
    let out = FLAG.replace_all(&out, "${1}${2}[REDACTED]");
    ASSIGN.replace_all(&out, "${1}${2}[REDACTED]").into_owned()
}

/// `text` with every URL in it cut at its query or fragment, wherever the URL
/// sits: at the start of a word, after `url=` or a quote, inside prose or a
/// JSON string. The URL ends at the first whitespace or closing quote, bracket
/// or parenthesis; what follows the `?` or `#` up to that end is dropped.
fn cut_url_queries(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let Some(pos) = ["https://", "http://"]
            .iter()
            .filter_map(|scheme| rest.find(scheme))
            .min()
        else {
            out.push_str(rest);
            return out;
        };
        let (before, from_scheme) = rest.split_at(pos);
        out.push_str(before);
        let end = from_scheme
            .find(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | ')' | ']' | '>' | '}'))
            .unwrap_or(from_scheme.len());
        let (url, tail) = from_scheme.split_at(end);
        out.push_str(url.split(['?', '#']).next().unwrap_or(url));
        rest = tail;
    }
}

/// `value` with every URL-shaped string cut at its query or fragment.
fn strip_url_queries(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::String(s) if s.contains("http://") || s.contains("https://") => {
            Value::String(cut_url_queries(s))
        }
        Value::Array(items) => Value::Array(items.iter().map(strip_url_queries).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), strip_url_queries(v)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The first line of a failure, plus the first stderr line when the text is a
/// command exit report -- that is where a program's own reason tends to be.
fn first_error_line(text: &str) -> String {
    // The line is read by the model and persisted with the session in a halt
    // summary, so it is scrubbed first: a command can print a token or a
    // user's own words on stderr.
    let scrubbed = crate::security::scrub::sanitize_text(text).value;
    let text = scrubbed.as_str();
    let first = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    let stderr = text
        .split_once("[stderr]\n")
        .and_then(|(_, tail)| tail.lines().find(|l| !l.trim().is_empty()))
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != first);
    let line = match stderr {
        Some(err) => format!("{first} — {err}"),
        None => first.to_owned(),
    };
    line.chars().take(RENDERED_CALL_CHARS).collect()
}

/// `summary` with the consecutive failing calls appended, oldest first, so the
/// report says what was tried and not only how the last attempt ended.
fn with_failing_calls(summary: String, failures: &std::collections::VecDeque<String>) -> String {
    if failures.is_empty() {
        return summary;
    }
    let mut out = summary;
    out.push_str("\n\nFailing calls, oldest first:");
    for (i, failure) in failures.iter().enumerate() {
        out.push_str(&format!("\n{}. {failure}", i + 1));
    }
    out
}

/// A stable, bounded fingerprint of a tool call's arguments for the identical-
/// repeat signature (hashed so a huge payload doesn't bloat the map/comparison).
fn args_fingerprint(arguments: &serde_json::Value) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    arguments.to_string().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Stable resource identity supplied by the call, excluding free-form queries,
/// prompts, credentials, and URL query parameters. An absent target remains
/// scoped to the operation, never to the changing argument fingerprint.
pub(super) fn failure_scope(tool: &str, arguments: &serde_json::Value) -> String {
    let mut scope = tool.to_owned();
    for field in [
        "account_id",
        "workspace_id",
        "app",
        "window_id",
        "resource",
        "endpoint",
        "url",
    ] {
        let value = match arguments.get(field) {
            Some(serde_json::Value::String(value)) if !value.is_empty() => value.clone(),
            Some(serde_json::Value::Number(value)) => value.to_string(),
            _ => continue,
        };
        if let Some(host_scope) = fetch_host_scope(tool, field, &value) {
            scope.push_str(&host_scope);
            continue;
        }
        scope.push(':');
        scope.push_str(field);
        scope.push('=');
        scope.push_str(&crate::util::truncate_with_ellipsis(
            value.split('?').next().unwrap_or(&value),
            120,
        ));
    }
    scope
}

/// Explicit recovery policy. Only recognised failures enter the classified
/// ledger; unknown prose continues through the established exact-repeat guard.
pub(super) fn recovery_policy(
    tool: &str,
    error: &str,
    body_level_failure: bool,
) -> Option<(&'static str, usize)> {
    let (class, budget) = classified_recovery_policy(tool, error, body_level_failure)?;
    // A connector, the hosted backend or the memory store refusing the
    // request (401/403, a missing or invalid key) says that service is not
    // available in this session. That is a tool to stop using, not a reason
    // to end the run: three web searches in one round came back `HTTP 401`
    // with no search provider configured, the `authentication` class halted
    // the turn on that first round, and a one-hour task ended after 101
    // seconds with the shell untouched. Steer the model off the tool once;
    // the ledger still halts if it insists.
    if matches!(class, "authentication" | "permission") && is_optional_service(tool) {
        return Some(("service_refused", 1));
    }
    // A path the model mistyped is a wrong call it can correct, not a missing
    // program: the classifier files `No such file or directory (os error 2)`
    // under `MissingApp`, which is right for a shell command and fatal for
    // `file_read`. One bad relative path ended a whole turn after two calls.
    // Any tool, not only the file tools: `use_skill` forwarding a screenshot
    // from a path that did not exist ("image forwarding failed: Failed to
    // resolve path …: No such file or directory") halted a one-hour task after
    // 92 seconds. A shell command's own "No such file" arrives as an exit
    // report, which never reaches this table.
    if class == "unsupported"
        && error
            .to_ascii_lowercase()
            .contains("no such file or directory")
    {
        return Some(("not_found", 1));
    }
    Some((class, budget))
}

/// Recovery budget for a tool rejecting its own arguments against its schema.
///
/// This is the most recoverable failure in the table: nothing ran, so there is
/// no side effect to reconcile, and the refusal hands the model the complete
/// expected schema (`tinyagents` `agent_loop/tools.rs`: "invalid arguments for
/// tool `X`: {detail}; expected schema: {…}"). It is a typo in one call, not a
/// broken world — unlike `not_found` or `unavailable`, which need the world to
/// change, and unlike a remote service's 400/422, which rejects a request the
/// model may have had every reason to send.
///
/// At a budget of 1 it ended terminal-bench 4.0 `vf2-speedup-networkx` at
/// 51/60 tests: the model omitted `edits[0].path` on two consecutive
/// `apply_patch` calls, while five others in the same run carried all three
/// fields — one of them 16,370 characters, seventeen times the size of the one
/// that failed. The omission was intermittent, not a size limit, so a further
/// attempt would most likely have landed. Three keeps it bounded; a call
/// repeated unchanged is still caught by the generic no-progress ladder.
const ARGUMENT_SCHEMA_RECOVERY: usize = 3;

/// Prefix of `tinytools::render_command_failure`, the one renderer every
/// shell-family tool uses for a command that ran and did not exit 0: an
/// exit-code (or signal) line, then the program's own stdout and stderr.
const COMMAND_EXIT_REPORT_PREFIX: &str = "Command failed (";

/// Whether `error` is a finished command's exit report rather than a failure
/// of the tool itself (a timeout, a policy refusal, a runtime that could not
/// be resolved), which the tools word differently.
fn is_command_exit_report(error: &str) -> bool {
    error.trim_start().starts_with(COMMAND_EXIT_REPORT_PREFIX)
}

/// Tools that reach a service on the user's behalf: an external connector,
/// the hosted backend, or the memory store. Work can go on without them.
fn is_optional_service(tool: &str) -> bool {
    use crate::core::all::DomainGroup;
    matches!(
        crate::tools::ops::tool_group(tool),
        DomainGroup::Integrations | DomainGroup::Memory | DomainGroup::Hosted
    )
}

fn classified_recovery_policy(
    tool: &str,
    error: &str,
    body_level_failure: bool,
) -> Option<(&'static str, usize)> {
    use crate::tools::status::ToolFailureClass as Class;
    if body_level_failure {
        return Some(("validation", 1));
    }
    // An unknown-tool answer is a wrong call the model can correct, and it
    // echoes the attempted name and every valid tool name. Keyword sniffing
    // below would read those names as the failure — `forbidden_tool` or a
    // name carrying `unauthorized` became `authentication`, a zero-retry
    // class, and halted the run on its first wrong guess.
    if error.trim_start().starts_with("unknown tool `") {
        return Some(("validation", 1));
    }
    // A command that ran and exited non-zero is reported as an exit-code line
    // followed by the program's own stdout and stderr. That output is data,
    // not a tool-layer verdict: keyword sniffing read `Update objects.md
    // (#401)` in a `git log | head` (exit 141, a harmless SIGPIPE) as a
    // credential failure, a zero-retry class, and ended the whole run on the
    // first call. The exit-code hint already steers the model, and the
    // generic no-progress ladder still bounds a command repeated unchanged.
    if is_command_exit_report(error) {
        return None;
    }
    // A module the host could not load stays unloaded until the app restarts,
    // so retrying the same tool cannot help. Steer the model off it once
    // rather than halting the run on the first call or spending a transient
    // budget on it (`restart the app to try again` read as recoverable).
    if error.contains(crate::tools::status::MODULE_FAULT_MARKER)
        && error.contains("restart the app to try again")
    {
        return Some(("unavailable", 1));
    }
    if let Some(policy) = fetched_site_policy(tool, error) {
        return policy;
    }
    // A tool-owned JSON error contract is less ambiguous than rendered prose.
    // Read only explicit status/code fields; arbitrary response data is not a
    // failure signal (this function is called only for `is_error` results).
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(error) {
        let status = value
            .get("status_code")
            .or_else(|| value.get("status"))
            .or_else(|| value.pointer("/error/status_code"))
            .and_then(serde_json::Value::as_u64);
        match status {
            Some(401) => return Some(("authentication", 0)),
            Some(403) => return Some(("permission", 0)),
            Some(400 | 422) => return Some(("validation", 1)),
            Some(429 | 500 | 502 | 503 | 504) => return Some(("transient", 2)),
            _ => {}
        }
        let code = value
            .get("code")
            .or_else(|| value.pointer("/error/code"))
            .and_then(serde_json::Value::as_str);
        match code {
            Some("PERMISSION_DENIED") => return Some(("permission", 0)),
            Some("UNAUTHENTICATED") => return Some(("authentication", 0)),
            Some("INVALID_ARGUMENT") => return Some(("validation", 1)),
            Some("WINDOW_NOT_FOUND") => return Some(("missing_window", 1)),
            Some("APP_NOT_FOUND") => return Some(("missing_app", 1)),
            Some("UNIMPLEMENTED") => return Some(("unsupported", 0)),
            Some("UNAVAILABLE" | "RESOURCE_EXHAUSTED") => return Some(("transient", 2)),
            _ => {}
        }
    }
    let class = crate::tools::status::classify(error, false).class;
    Some(match class {
        Class::MissingPermission => ("permission", 0),
        Class::BadCredentials => ("authentication", 0),
        Class::BlockedByPolicy | Class::Denied | Class::ApprovalExpired => ("policy", 0),
        Class::Unsupported | Class::MissingApp => ("unsupported", 0),
        Class::NotFound
            if tool.contains("desktop") && error.to_ascii_lowercase().contains("window") =>
        {
            ("missing_window", 1)
        }
        Class::NotFound => ("not_found", 1),
        Class::ServiceUnavailable | Class::ModelConnection => ("transient", 2),
        Class::Timeout
            if matches!(
                tool,
                "web_search" | "web_fetch" | "file_read" | "list_files" | "desktop_list_windows"
            ) =>
        {
            ("transient", 2)
        }
        // A local command killed by the shell's own timeout is still uncertain
        // (it may have partly run), but it is inspectable: the model can check
        // the filesystem or re-run a smaller, bounded step. Halting the whole
        // turn on the first one threw away every earlier result for what is
        // usually a slow read (a `whois`/`dig` loop). It gets one recovery
        // attempt, steered by a reconcile-first nudge, and halts on a second.
        // Remote actions (`gmail_send`, payments, …) stay at zero: a retry
        // there can repeat an effect the agent cannot observe.
        Class::Timeout if tool == "shell" => ("uncertain_side_effect", 1),
        Class::Timeout => ("uncertain_side_effect", 0),
        Class::Unknown if is_recoverable_tool_failure(error) => ("transient", 2),
        // Its own class, not the `validation` bucket: the ledger keys on
        // (class, operation, scope), so pooling this with `unknown tool` and
        // `validate_workflow`'s invalid graphs would hand those a budget they
        // should not have. A wrong tool name does not become right, and a graph
        // the model cannot fix should still stop.
        Class::Unknown
            if error.to_ascii_lowercase().contains("schema validation")
                || error.to_ascii_lowercase().contains("invalid arguments") =>
        {
            ("invalid_arguments", ARGUMENT_SCHEMA_RECOVERY)
        }
        Class::Unknown => return None,
    })
}

/// Detect a **body-level** failure from `validate_workflow` / `dry_run_workflow`
/// (issue: flows breaker doesn't see repeated invalid-graph loops). Both tools
/// report an invalid graph / aborted sandbox run via `ToolResult::success` with
/// a JSON body carrying top-level `"ok": false`
/// (`crates/openhuman-core/src/flows/builder_tools.rs`) rather than `ToolResult::error` — so
/// `result.error` stays `None` and the no-progress breaker below never counts
/// the repeat as a failure, letting a graph the model can't fix burn the whole
/// iteration budget instead of tripping the same nudge/halt ladder.
///
/// Scoped to exactly these two tool names: a generic `"ok": false` in some other
/// tool's JSON body may be legitimate data (not a failure signal), so this must
/// not reinterpret arbitrary tool output. Tolerant of non-JSON or missing `ok`
/// content — returns `false` rather than guessing.
pub(crate) fn is_body_level_failure(name: &str, content: &str) -> bool {
    if name != "validate_workflow" && name != "dry_run_workflow" {
        return false;
    }
    match serde_json::from_str::<serde_json::Value>(content) {
        Ok(serde_json::Value::Object(map)) => {
            matches!(map.get("ok"), Some(serde_json::Value::Bool(false)))
        }
        _ => false,
    }
}

#[async_trait]
impl Middleware<(), crate::agent::tinyagents::host::OpenHumanRunContext>
    for RepeatedToolFailureMiddleware
{
    fn name(&self) -> &str {
        "repeated_tool_failure"
    }

    // Failure accounting and corrective nudges must observe tool outcomes even
    // when an earlier middleware has already requested a control action.
    fn is_observer(&self) -> bool {
        true
    }

    async fn before_tool(
        &self,
        _ctx: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        _state: &(),
        call: &mut TaToolCall,
    ) -> TaResult<()> {
        // The tool result carries no arguments, so capture a fingerprint here and
        // correlate it by call_id in `after_tool`.
        if let Ok(mut sigs) = self.arg_sigs.lock() {
            sigs.insert(call.id.clone(), args_fingerprint(&call.arguments));
        }
        if let Ok(mut scopes) = self.target_scopes.lock() {
            scopes.insert(call.id.clone(), failure_scope(&call.name, &call.arguments));
        }
        if let Ok(mut calls) = self.recent_calls.lock() {
            calls.insert(call.id.clone(), render_call(&call.name, &call.arguments));
        }
        Ok(())
    }

    async fn after_tool(
        &self,
        _ctx: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        _state: &(),
        invocation: &ToolInvocationIdentity,
        result: &mut TaToolResult,
    ) -> TaResult<()> {
        let tool_name = invocation.tool_name();
        let content = crate::agent::tinyagents::middleware::tool_result_text(result);
        let arg_fp = self
            .arg_sigs
            .lock()
            .ok()
            .and_then(|mut sigs| sigs.remove(&invocation.call_id().to_string()))
            .unwrap_or_default();
        let scope = self
            .target_scopes
            .lock()
            .ok()
            .and_then(|mut scopes| scopes.remove(&invocation.call_id().to_string()))
            .unwrap_or_else(|| tool_name.to_owned());
        // A result the repeat guard answered itself (blocked/halted without
        // running the tool) is the guard's verdict, not the tool failing, so it
        // must not feed the failure ladder.
        if let Some(marker) = repeat_guard_marker(result) {
            tracing::debug!(
                tool = tool_name,
                marker,
                "[tinyagents::mw] skipping repeat-guard result in failure ladder"
            );
            return Ok(());
        }
        let step = self.step.fetch_add(1, Ordering::SeqCst) + 1;

        // Body-level failure signal: `validate_workflow` / `dry_run_workflow`
        // report an invalid graph via a `success` result whose JSON body carries
        // `"ok": false` — see `is_body_level_failure`. Only meaningful when
        // `result.error` is `None`; when both are set, `result.error` already
        // drives every check below, so this never double-counts one failure.
        let body_level_failure = !result.is_error && is_body_level_failure(tool_name, &content);

        // Combined failure text for classification: the model-facing content plus
        // the (redundant but authoritative) error field. Both are scanned for the
        // policy / terminal-inference / recoverable markers below.
        let failure_text = match result.is_error {
            true => content.clone(),
            false if body_level_failure => content.clone(),
            false => String::new(),
        };
        let heuristic_failure_text = heuristic_text(tool_name, &failure_text);

        // What this call was, for a halt summary; the consecutive-failure list
        // mirrors the ladder: it grows on every failure and empties on any success.
        let rendered = self
            .recent_calls
            .lock()
            .ok()
            .and_then(|mut calls| calls.remove(&invocation.call_id().to_string()))
            .unwrap_or_else(|| tool_name.to_owned());
        if let Ok(mut failures) = self.recent_failures.lock() {
            if result.is_error || body_level_failure {
                failures.push_back(format!(
                    "`{tool_name}`: {rendered} → {}",
                    first_error_line(&failure_text)
                ));
                while failures.len() > RENDERED_FAILURES {
                    failures.pop_front();
                }
            } else {
                failures.clear();
            }
        }

        if !result.is_error && !body_level_failure {
            // Only a successful observation against this operation and scope
            // demonstrates that its blocker changed. Unrelated successes do not.
            for class in [
                "permission",
                "authentication",
                "site_refused",
                "policy",
                "unsupported",
                "missing_window",
                "missing_app",
                "not_found",
                "transient",
                "uncertain_side_effect",
                "validation",
                "unavailable",
                "service_refused",
                "invalid_arguments",
            ] {
                self.classified
                    .clear(&ClassifiedFailure::new(class, tool_name, &scope));
            }
        } else if !is_repeat_call_exempt(tool_name) {
            if let Some((class, budget)) =
                recovery_policy(tool_name, &failure_text, body_level_failure)
            {
                let key = ClassifiedFailure::new(class, tool_name, &scope);
                if let NoProgress::Halt(mut summary) = self.classified.record(&key, budget) {
                    tracing::warn!(
                        tool = tool_name,
                        class,
                        budget,
                        "[tinyagents::mw] classified failure budget exhausted"
                    );
                    if class == "uncertain_side_effect" {
                        summary.push_str(" The action may already have happened; reconcile its external state before any retry.");
                    }
                    if let Ok(mut slot) = self.halt_summary.lock() {
                        *slot = Some(self.halt_with(summary));
                    }
                    self.handle.send(SteeringCommand::Pause);
                    self.tracker.reset();
                    return Ok(());
                }
                if matches!(
                    class,
                    "missing_window"
                        | "missing_app"
                        | "validation"
                        | "invalid_arguments"
                        | "uncertain_side_effect"
                        | "unavailable"
                        | "service_refused"
                ) {
                    let instruction = match class {
                        "service_refused" => format!(
                            "The `{tool_name}` tool cannot be used in this session: the service refused the request ({}). Do not call `{tool_name}` again; continue with your other tools.",
                            first_error_line(&failure_text)
                        ),
                        "validation" => "The last call failed validation. Correct its schema or arguments once before trying again.".to_owned(),
                        "invalid_arguments" => format!(
                            "The `{tool_name}` call was rejected before it ran: its arguments did not match the tool's schema ({}). Read the tool's parameters and correct the call; do not resend it unchanged.",
                            first_error_line(&failure_text)
                        ),
                        "uncertain_side_effect" => "The last command timed out and was killed; it may have partly run. Check its effect before repeating anything, then retry at most once as a smaller, bounded step (fewer items per call, a per-item timeout such as `timeout 5`, or background it and poll).".to_owned(),
                        "unavailable" => format!("The `{tool_name}` tool is unavailable for the rest of this run: a module it needs failed to load and will not recover until the app restarts. Do not call `{tool_name}` again; continue with your other tools."),
                        _ => "The desktop target was not found. Rediscover the current app and window once before trying again.".to_owned(),
                    };
                    tracing::debug!(
                        tool = tool_name,
                        class,
                        "[tinyagents::mw] classified failure within budget — nudging recovery"
                    );
                    self.queue_nudge(instruction);
                }
                // The classified budget owns this known blocker. In particular,
                // a different query must not reset its count or trigger a
                // competing exact-repeat nudge.
                return Ok(());
            }
        }

        // ── Part 5 (#3104): terminal delegated-inference fast-halt ──────────────
        // A permanent inference failure (out of budget / provider-config rejection)
        // surfaced by a delegated sub-agent cannot be recovered by retrying — the
        // budget is account-wide and the model/provider config is shared by every
        // (sub-)agent. Halt on the FIRST occurrence with an actionable root cause,
        // *before* the count-based thresholds, because the orchestrator otherwise
        // re-emits the doomed step under varied delegation-tool names so the
        // identical-retry threshold never trips in time.
        // A command's exit report carries the program's output, which can quote
        // a provider error (a script calling an API) without the agent's own
        // inference having failed.
        if result.is_error && !is_command_exit_report(&failure_text) {
            if let Some(kind) = terminal_inference_failure_kind(heuristic_failure_text) {
                tracing::warn!(
                    tool = tool_name,
                    kind = ?kind,
                    "[tinyagents::mw] terminal delegated-inference failure — halting on first occurrence with root cause"
                );
                if let Ok(mut slot) = self.halt_summary.lock() {
                    *slot = Some(self.halt_with(terminal_inference_halt_summary(
                        kind,
                        tool_name,
                        &failure_text,
                    )));
                }
                self.handle.send(SteeringCommand::Pause);
                self.tracker.reset();
                self.reset_recoverable_streak();
                return Ok(());
            }
        }

        // A hard policy rejection is marked in the tool output; it can never
        // succeed when re-issued unchanged, so the crate ladder trips it faster
        // (its `HARD_REJECT_HALT_THRESHOLD` of 2). Both the read-only/forbidden
        // block (`POLICY_BLOCKED_MARKER`) and the approval denial / TTL expiry
        // (`POLICY_DENIED_MARKER`) are deterministic — restore the 2-repeat
        // fast-trip for BOTH (issue #4463 part 6: denied had drifted to the
        // generic 3).
        let policy_marked = |s: &str| {
            s.contains(crate::security::POLICY_BLOCKED_MARKER)
                || s.contains(crate::security::POLICY_DENIED_MARKER)
        };
        let hard_reject = policy_marked(&content);

        // ── Part 4: recoverable-failure headroom ────────────────────────────────
        // Transient failures (timeouts, connection resets, rate limits, 5xx) get
        // the legacy extended headroom instead of the crate's deterministic 3/6.
        // Route them to the recoverable ladder; a success or a non-recoverable
        // failure resets that streak and feeds the crate tracker as before.
        // A finished command's exit report is the program's output, so a test
        // run that prints `timed out` or `connection refused` is not a
        // transient tool failure. Its identical-repeat count would otherwise
        // persist across the turn and halt an edit-and-rerun loop on the same
        // test command; the crate tracker below resets on any success instead.
        let recoverable = result.is_error
            && !hard_reject
            && !is_command_exit_report(&failure_text)
            && (is_recoverable_tool_failure(heuristic_failure_text)
                || matches!(
                    crate::tools::status::classify(heuristic_failure_text, false).class,
                    crate::tools::status::ToolFailureClass::Timeout
                        | crate::tools::status::ToolFailureClass::ServiceUnavailable
                        | crate::tools::status::ToolFailureClass::ModelConnection
                ));
        if recoverable {
            // A poll tool's contract is the identical repeat (see
            // [`is_repeat_call_exempt`]), and the thing it repeats on is a
            // *timeout* — which lands here as a recoverable failure. Counting
            // those toward the identical-argument headroom halts exactly the
            // loop the tool is documented to ask for: a sub-agent that outlives
            // eight wait windows killed the turn, discarding work it had already
            // done. `RepeatProgressMiddleware` already honours this exemption on
            // the success side; the failure ladder must agree, or the exemption
            // only holds while the wait happens to return early.
            if is_repeat_call_exempt(tool_name) {
                return Ok(());
            }
            if let Some(summary) = self.record_recoverable(tool_name, &arg_fp, &failure_text) {
                tracing::warn!(
                    tool = tool_name,
                    "[tinyagents::mw] recoverable-failure headroom exhausted — halting run so the root cause surfaces"
                );
                if let Ok(mut slot) = self.halt_summary.lock() {
                    *slot = Some(self.halt_with(summary));
                }
                self.handle.send(SteeringCommand::Pause);
                self.reset_recoverable_streak();
            }
            // Recoverable failures never feed the crate tracker — its fixed 3/6
            // backstop would halt them before the extended headroom is spent.
            return Ok(());
        }
        // Success or non-recoverable failure: clear the recoverable streak (its
        // per-signature counts persist across the turn) before the crate tracker
        // handles the deterministic 3/6 + hard-reject-2 path below.
        self.reset_recoverable_streak();

        // Union the body-level `ok:false` signal with the existing `error.is_some()`
        // predicate so the crate tracker (which reads `attempt.error` as its sole
        // success/failure signal — `None` means "progress was made, reset every
        // counter") sees the repeat as a failure and feeds it into the same
        // nudge/halt ladder as a real tool error.
        // A finished command that exited non-zero is the program's answer, and
        // a different command's non-zero exit is new information, not a
        // repeat: `pip install` failing to build, `g++` turning out to be
        // missing, a `which cc` that finds nothing. Six such answers in a row
        // ended one turn 39 s into a 60-minute budget as "no progress". Only
        // the same command failing again counts toward the ladder; a different
        // one resets it the way a success would. A loop of varied commands is
        // still bounded by the call cap and the clock.
        let exit_report = result.is_error && !hard_reject && is_command_exit_report(&failure_text);
        let same_command_again = exit_report && {
            let key = format!("{tool_name}\u{1f}{arg_fp}");
            let mut last = self.last_exit_report.lock().ok();
            let repeat = last
                .as_deref()
                .is_some_and(|l| l.as_deref() == Some(key.as_str()));
            if let Some(slot) = last.as_mut() {
                **slot = Some(key);
            }
            repeat
        };
        if !exit_report {
            if let Ok(mut last) = self.last_exit_report.lock() {
                *last = None;
            }
        }
        if exit_report && !same_command_again {
            // A new command: the ladder starts over, as after a success, and
            // the list a halt would print starts with this call.
            self.tracker.reset();
            if let Ok(mut failures) = self.recent_failures.lock() {
                let newest = failures.pop_back();
                failures.clear();
                failures.extend(newest);
            }
        }
        let attempt_error: Option<&str> = match result.is_error {
            true => Some(failure_text.as_str()),
            false if body_level_failure => Some(failure_text.as_str()),
            false => None,
        };
        let attempt = ToolAttempt {
            tool: tool_name,
            arg_fingerprint: &arg_fp,
            error: attempt_error,
            hard_reject,
            // The unknown-tool recovery sentinel is a C3 concern; today every
            // failure feeds the generic backstop exactly as the legacy ladder did.
            recoverable_miss: false,
        };

        match self.tracker.record(step, &attempt) {
            NoProgress::Continue => {}
            NoProgress::Nudge(instruction) => {
                tracing::warn!(
                    tool = tool_name,
                    step,
                    hard_reject,
                    "[tinyagents::mw] no-progress nudge — steering the model to change strategy before the retry cap"
                );
                // Request-scoped, not steering: no run policy can reject it (a
                // `Redirect` nudge aborted interactive turns, #4473), and it is
                // never committed into durable history (#6725).
                self.queue_nudge(instruction);
            }
            NoProgress::Halt(summary) => {
                // #4092: if the blocker is user-actionable (a missing connection),
                // escalate with a concrete ask instead of the crate's generic
                // "unreachable environment, report back" summary.
                // A command printing `not connected` is not a missing integration.
                let escalation = (!is_command_exit_report(&content))
                    .then(|| user_actionable_escalation(tool_name, &content))
                    .flatten();
                let user_actionable = escalation.is_some();
                let summary = escalation.unwrap_or(summary);
                tracing::warn!(
                    tool = tool_name,
                    step,
                    hard_reject,
                    user_actionable,
                    "[tinyagents::mw] repeated tool failure — halting run so the root cause surfaces"
                );
                if let Ok(mut slot) = self.halt_summary.lock() {
                    *slot = Some(self.halt_with(summary));
                }
                // Pause at the top of the next iteration (before the next model
                // call), matching the stop-hook / cap pause path. Reset so a
                // resumed run does not immediately re-pause on the latched state
                // (the crate also resets internally on a halt; this is explicit
                // and idempotent).
                self.handle.send(SteeringCommand::Pause);
                self.tracker.reset();
            }
        }
        Ok(())
    }
}
