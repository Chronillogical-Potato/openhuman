use super::*;

use super::super::repeated_failure::recovery_policy;

/// The tengo DeepSWE task: `git log --oneline | head` exits 141 (SIGPIPE when
/// `head` closes the pipe), and one commit subject is `Update objects.md (#401)`.
fn sigpipe_git_log_report() -> String {
    tinytools::render_command_failure(
        Some(141),
        "ef59933 bench-baseline\n3cad0da calculate compiled size (#456)\n2edd39e Update objects.md (#401)",
        "",
    )
}

async fn run_shell(
    mw: &RepeatedToolFailureMiddleware,
    id: &str,
    command: &str,
    mut result: TaToolResult,
) {
    let mut call = TaToolCall::new(id, "shell", json!({ "command": command }));
    mw.before_tool(&mut ctx(), &(), &mut call).await.unwrap();
    mw.after_tool(&mut ctx(), &(), &invocation(id, "shell"), &mut result)
        .await
        .unwrap();
}

#[test]
fn a_command_exit_report_is_program_output_not_a_failure_class() {
    assert_eq!(
        recovery_policy("shell", &sigpipe_git_log_report(), false),
        None
    );
    for stdout in [
        "HTTP/1.1 403 Forbidden",
        "test_login ... Unauthorized",
        "pytest: 3 tests timed out",
        "error: Permission denied (os error 13)",
    ] {
        let report = tinytools::render_command_failure(Some(1), stdout, "");
        assert_eq!(recovery_policy("shell", &report, false), None, "{stdout}");
    }
    // A missing program is still reported with its exit code and hint, which
    // steers the model; it no longer ends the run on the first missing tool.
    let missing = tinytools::render_command_failure(Some(127), "", "bash: jq: command not found");
    assert_eq!(recovery_policy("shell", &missing, false), None);
    let signalled = tinytools::render_command_failure(None, "partial", "");
    assert_eq!(recovery_policy("shell", &signalled, false), None);
}

#[test]
fn tool_layer_shell_failures_are_still_classified() {
    // Worded by the shell tool itself, not a finished command's report.
    assert_eq!(
        recovery_policy("shell", "Command timed out after 60s and was killed", false),
        Some(("uncertain_side_effect", 1))
    );
    assert_eq!(
        recovery_policy("gmail_send", "401 Unauthorized", false),
        Some(("authentication", 0))
    );
}

#[tokio::test]
async fn a_sigpipe_with_401_in_its_output_does_not_halt_the_run() {
    let handle = SteeringHandle::allow_all();
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mw = RepeatedToolFailureMiddleware::new(handle.clone(), 3, slot.clone());
    run_shell(
        &mw,
        "log-1",
        "git log --oneline | head",
        failing_result("shell", &sigpipe_git_log_report()),
    )
    .await;
    assert_eq!(drain_pause_count(&handle), 0);
    assert!(slot.lock().unwrap().is_none());
}

/// An edit-and-rerun loop reruns the same test command many times. Output
/// that mentions a timeout used to route it to the recoverable ladder, whose
/// identical-repeat count survives successes and halted the run at 8.
#[tokio::test]
async fn rerunning_a_failing_test_between_edits_never_halts() {
    let handle = SteeringHandle::allow_all();
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mw = RepeatedToolFailureMiddleware::new(handle.clone(), 3, slot.clone());
    let report = tinytools::render_command_failure(
        Some(1),
        "FAILED tests/test_consumer.py::test_priority - TimeoutError: timed out",
        "",
    );
    for round in 0..12 {
        run_shell(
            &mw,
            &format!("test-{round}"),
            "python -m pytest tests/test_consumer.py",
            failing_result("shell", &report),
        )
        .await;
        run_shell(
            &mw,
            &format!("edit-{round}"),
            "sed -i 's/a/b/' kombu/consumer.py",
            tool_result("shell", ""),
        )
        .await;
    }
    assert_eq!(drain_pause_count(&handle), 0);
    assert!(slot.lock().unwrap().is_none());
}

/// The same failing command repeated with nothing in between is still a loop,
/// and the generic no-progress ladder still ends it.
#[tokio::test]
async fn an_identical_failing_command_repeated_unchanged_still_halts() {
    let handle = SteeringHandle::allow_all();
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mw = RepeatedToolFailureMiddleware::new(handle.clone(), 3, slot.clone());
    let report = tinytools::render_command_failure(Some(2), "", "make: *** No rule to make target");
    for id in ["make-1", "make-2", "make-3", "make-4"] {
        run_shell(&mw, id, "make build", failing_result("shell", &report)).await;
    }
    assert_eq!(drain_pause_count(&handle), 1);
}

const MODULE_FAULT: &str =
    "Failed to resolve command runtime: module 'tinyruntime' could not be loaded \
     from the installer bundle: module `ubuntu-22.04-x86_64` refused: module directory is owned by \
     another user. This is terminal for the running process; restart the app to try again";

#[test]
fn a_faulted_module_is_unavailable_not_transient() {
    assert_eq!(
        recovery_policy("web_search_tool", MODULE_FAULT, false),
        Some(("unavailable", 1))
    );
}

#[test]
fn quoted_module_fault_marker_is_not_enough_to_mark_a_tool_unavailable() {
    let quoted = format!(
        "search result quoted: {}",
        crate::tools::status::MODULE_FAULT_MARKER
    );
    assert_eq!(recovery_policy("web_search_tool", &quoted, false), None);
}

#[test]
fn the_module_fault_marker_matches_the_producer_wording() {
    assert!(MODULE_FAULT.contains(crate::tools::status::MODULE_FAULT_MARKER));
}

#[tokio::test]
async fn a_faulted_module_tool_is_steered_away_once_then_halts() {
    let handle = SteeringHandle::allow_all();
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mw = RepeatedToolFailureMiddleware::new(handle.clone(), 3, slot.clone());
    let mut call = TaToolCall::new("search-1", "web_search_tool", json!({ "query": "one" }));
    mw.before_tool(&mut ctx(), &(), &mut call).await.unwrap();
    let mut result = failing_result("web_search_tool", MODULE_FAULT);
    mw.after_tool(
        &mut ctx(),
        &(),
        &invocation("search-1", "web_search_tool"),
        &mut result,
    )
    .await
    .unwrap();
    assert_eq!(drain_pause_count(&handle), 0);
    let nudges = drain_nudge_messages(&mw);
    assert_eq!(nudges.len(), 1, "{nudges:?}");
    assert!(nudges[0].contains("`web_search_tool`"), "{}", nudges[0]);
    assert!(nudges[0].contains("Do not call"), "{}", nudges[0]);

    let mut call = TaToolCall::new("search-2", "web_search_tool", json!({ "query": "two" }));
    mw.before_tool(&mut ctx(), &(), &mut call).await.unwrap();
    let mut result = failing_result("web_search_tool", MODULE_FAULT);
    mw.after_tool(
        &mut ctx(),
        &(),
        &invocation("search-2", "web_search_tool"),
        &mut result,
    )
    .await
    .unwrap();
    assert_eq!(drain_pause_count(&handle), 1);
    let summary = slot.lock().unwrap().clone().unwrap();
    assert!(summary.contains("unavailable"), "{summary}");
}

/// A result the repeat guard answered itself (blocked/halted without running
/// the tool) is the guard's verdict, not the tool failing: it must stay out of
/// the failure ladder even when its text would classify as a hard failure.
#[tokio::test]
async fn a_repeat_guard_result_never_feeds_the_failure_ladder() {
    let handle = SteeringHandle::allow_all();
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mw = RepeatedToolFailureMiddleware::new(handle.clone(), 3, slot.clone());
    for round in 0..6 {
        let mut result = failing_result("gmail_send", "401 Unauthorized");
        result.metadata = Some(json!({ "tinyagents.repeat_guard": "blocked" }));
        let id = format!("guard-{round}");
        let mut call = TaToolCall::new(&id, "gmail_send", json!({ "to": "a@b.c" }));
        mw.before_tool(&mut ctx(), &(), &mut call).await.unwrap();
        mw.after_tool(&mut ctx(), &(), &invocation(&id, "gmail_send"), &mut result)
            .await
            .unwrap();
    }
    assert_eq!(drain_pause_count(&handle), 0);
    assert!(slot.lock().unwrap().is_none());
}

/// A halt that names only the last error leaves the failing commands
/// unrecoverable: one run was stopped after six different submissions and kept
/// no record of what any of them sent. The summary must list the calls it
/// counted, oldest first, each with its own error.
#[tokio::test]
async fn a_no_progress_halt_lists_the_calls_it_counted() {
    // Only a command repeated unchanged climbs the ladder (a different
    // command's non-zero exit is new information and restarts it), so the
    // list a halt prints is the identical attempts, oldest first.
    let handle = SteeringHandle::allow_all();
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mw = RepeatedToolFailureMiddleware::new(handle.clone(), 3, slot.clone());
    let report =
        tinytools::render_command_failure(Some(1), "", "Expecting value: line 1 column 1 (char 0)");
    for i in 0..3 {
        run_shell(
            &mw,
            &format!("post-{i}"),
            "python3 submit.py --claim CLM-2600",
            failing_result("shell", &report),
        )
        .await;
    }
    let summary = slot
        .lock()
        .unwrap()
        .clone()
        .expect("the same failing call three times halts the run");
    assert!(
        summary.contains("Failing calls, oldest first:"),
        "{summary}"
    );
    assert!(
        summary.contains("1. `shell`: python3 submit.py --claim CLM-2600"),
        "the oldest counted call is listed first: {summary}"
    );
    assert!(
        summary.contains("3. `shell`: python3 submit.py --claim CLM-2600"),
        "the call that tripped the halt is listed last: {summary}"
    );
    assert!(
        summary.matches("Expecting value").count() >= 3,
        "each entry carries the program's own error line: {summary}"
    );
}

/// A success empties the list: only the failures the ladder is still counting
/// belong in a halt, and a long list of stale ones would misdirect the reader.
#[tokio::test]
async fn a_success_between_failures_clears_the_listed_calls() {
    let handle = SteeringHandle::allow_all();
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mw = RepeatedToolFailureMiddleware::new(handle.clone(), 3, slot.clone());
    let report = tinytools::render_command_failure(Some(1), "", "boom");
    for i in 0..2 {
        run_shell(&mw, &format!("a-{i}"), "false-a", failing_result("shell", &report)).await;
    }
    run_shell(&mw, "ok", "true", TaToolResult::success("fine")).await;
    for i in 0..3 {
        run_shell(&mw, &format!("b-{i}"), "false-b", failing_result("shell", &report)).await;
    }
    let summary = slot
        .lock()
        .unwrap()
        .clone()
        .expect("halts on the second streak");
    assert!(
        !summary.contains("false-a"),
        "the pre-success streak is gone: {summary}"
    );
    assert!(
        summary.contains("1. `shell`: false-b") && summary.contains("3. `shell`: false-b"),
        "{summary}"
    );
}

/// The listed calls are read by the model and persisted with the session, so
/// they carry what identifies a call and nothing that should not travel: a
/// URL's query (which can hold a token or the user's own words) and API-key
/// shapes in a command are both gone, while host, path and command remain.
#[tokio::test]
async fn listed_calls_carry_no_query_strings_or_secrets() {
    let handle = SteeringHandle::allow_all();
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mw = RepeatedToolFailureMiddleware::new(handle.clone(), 3, slot.clone());
    // Built here, not written as a literal: a key-shaped literal in source
    // trips secret scanners, and the scrubber only sees the runtime string.
    let fake_key = format!("sk-ant-{}{}", "abcdefghijklmnopqrstuvwxyz0123456789", "ABCDEF");
    run_shell(
        &mw,
        "key-0",
        &format!("curl -H 'x-api-key: {fake_key}' https://api.example.test/v1/run?debug=1"),
        failing_result("shell", "boom"),
    )
    .await;
    for i in 1..6 {
        let id = format!("fetch-{i}");
        let mut call = TaToolCall::new(
            &id,
            "web_fetch",
            json!({ "url": format!("https://example.test/p{i}?token=abc{i}") }),
        );
        mw.before_tool(&mut ctx(), &(), &mut call).await.unwrap();
        let mut result = failing_result("web_fetch", "boom");
        mw.after_tool(&mut ctx(), &(), &invocation(&id, "web_fetch"), &mut result)
            .await
            .unwrap();
    }
    let summary = slot
        .lock()
        .unwrap()
        .clone()
        .expect("six varied failures halt");
    assert!(
        summary.contains("https://example.test/p1"),
        "host and path kept: {summary}"
    );
    assert!(
        !summary.contains("token=abc"),
        "query strings dropped: {summary}"
    );
    assert!(
        !summary.contains("?debug=1"),
        "query dropped inside a command too: {summary}"
    );
    assert!(
        !summary.contains("abcdefghijklmnopqrstuvwxyz0123456789"),
        "key scrubbed: {summary}"
    );
    assert!(
        summary.contains("curl -H"),
        "the command itself is still named: {summary}"
    );
}

fn exit_report(code: i32, stdout: &str, stderr: &str) -> String {
    tinytools::render_command_failure(Some(code), stdout, stderr)
}

#[tokio::test]
async fn six_different_failing_commands_are_investigation_not_a_loop() {
    // train-fasttext, 2026-10-08: `pip install fasttext` failed to build, `g++`
    // was missing, an environment survey exited 2, a build-backend probe
    // failed — six distinct non-zero exits, each telling the model something
    // new — and the ladder halted the turn 39 s into a 60-minute budget.
    let handle = SteeringHandle::allow_all();
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mw = RepeatedToolFailureMiddleware::new(handle.clone(), 3, slot.clone());
    let attempts = [
        ("pip install fasttext", exit_report(1, "", "RuntimeError: Unsupported compiler")),
        ("pip install fasttext-wheel", exit_report(1, "", "No matching distribution")),
        ("g++ --version", exit_report(127, "", "g++: command not found")),
        ("cat /etc/os-release; which cc gcc clang", exit_report(2, "Debian 12", "")),
        ("python -c 'import setuptools.build_meta'", exit_report(1, "", "BackendUnavailable")),
        ("apt-get install -y g++", exit_report(100, "", "E: Unable to locate package")),
        ("pip download fasttext --no-deps", exit_report(1, "", "network unreachable")),
    ];
    for (i, (command, report)) in attempts.iter().enumerate() {
        run_shell(&mw, &format!("probe-{i}"), command, failing_result("shell", report)).await;
    }
    assert_eq!(drain_pause_count(&handle), 0, "distinct failing commands never halt");
    assert!(slot.lock().unwrap().is_none(), "no halt summary");
}

#[tokio::test]
async fn the_same_failing_command_three_times_still_halts() {
    let handle = SteeringHandle::allow_all();
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mw = RepeatedToolFailureMiddleware::new(handle.clone(), 3, slot.clone());
    for i in 0..3 {
        run_shell(
            &mw,
            &format!("same-{i}"),
            "pip install fasttext",
            failing_result("shell", &exit_report(1, "", "RuntimeError: Unsupported compiler")),
        )
        .await;
    }
    assert_eq!(drain_pause_count(&handle), 1, "an identical failing command still trips the ladder");
    let summary = slot.lock().unwrap().clone().unwrap();
    assert!(summary.contains("pip install fasttext"), "{summary}");
}

#[tokio::test]
async fn a_different_command_between_repeats_resets_the_identical_count() {
    let handle = SteeringHandle::allow_all();
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mw = RepeatedToolFailureMiddleware::new(handle.clone(), 3, slot.clone());
    let same = || failing_result("shell", &exit_report(1, "", "boom"));
    run_shell(&mw, "a-1", "make", same()).await;
    run_shell(&mw, "a-2", "make", same()).await;
    run_shell(&mw, "b-1", "ls build/", failing_result("shell", &exit_report(2, "", "No such file"))).await;
    run_shell(&mw, "a-3", "make", same()).await;
    run_shell(&mw, "a-4", "make", same()).await;
    assert_eq!(drain_pause_count(&handle), 0, "the streak restarted after `ls build/`");
    run_shell(&mw, "a-5", "make", same()).await;
    assert_eq!(drain_pause_count(&handle), 1, "three identical failures in a row halt");
}
