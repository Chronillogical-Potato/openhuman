//! Running the staged workers: the serial shared-workspace-write fallback,
//! the bounded `map_reduce` parallel fanout, and the single-worker execution
//! (`run_subagent` + post-run worktree status snapshot) both paths share.

use std::path::PathBuf;

use tinyagents_graph::parallel::{map_reduce, FailurePolicy, ParallelOptions};
use tinyagents_harness::{CancellationToken, TinyAgentsError};

use crate::agent::subagent_host::{
    run_subagent_with_parent, SubagentRunOptions, SubagentRunStatus,
};

use super::staging::WorkerDispatchMode;
use super::types::{ParallelAgentResult, ParallelAgentStatus, SpawnParallelWorker};

pub(crate) async fn run_spawn_parallel_workers(
    prepared: Vec<SpawnParallelWorker>,
    action_root: Option<PathBuf>,
    cancel: CancellationToken,
    run_context: crate::agent::tinyagents::host::OpenHumanRunContext,
    live_parent: &tinyagents_harness::context::RunContext<
        crate::agent::tinyagents::host::OpenHumanRunContext,
    >,
) -> tinyagents_harness::Result<Vec<ParallelAgentResult>> {
    let n = prepared.len();
    let serial_write_count = prepared
        .iter()
        .filter(|worker| {
            matches!(
                worker.dispatch_mode,
                WorkerDispatchMode::SerialSharedWorkspaceWrite
            )
        })
        .count();
    if serial_write_count > 0 {
        tracing::debug!(
            target: "orchestration",
            workers = n,
            serial_write_count,
            "[orchestration] running serial fallback for shared-workspace write fan-out"
        );
        let mut results = Vec::with_capacity(n);
        for worker in prepared {
            if cancel.is_cancelled() {
                tracing::debug!(
                    target: "orchestration",
                    "[orchestration] spawn_parallel serial fan-out cancelled before next worker"
                );
                return Err(TinyAgentsError::Cancelled);
            }
            results.push(
                run_one_parallel_task(
                    worker,
                    action_root.clone(),
                    run_context.child(),
                    live_parent,
                )
                .await,
            );
        }
        return Ok(results);
    }

    let max_concurrency = prepared.len().max(1);
    let action_root_for_workers = action_root.clone();
    let run_context_for_workers = run_context.clone();
    tracing::debug!(
        target: "orchestration",
        workers = n,
        max_concurrency,
        "[orchestration] running parallel fan-out on tinyagents map_reduce (spawn_parallel_agents)"
    );
    let options = ParallelOptions::default()
        .with_max_concurrency(max_concurrency)
        .with_failure_policy(FailurePolicy::CollectAll)
        .with_cancellation(cancel);
    let outcome = map_reduce(prepared, options, move |_i, worker| {
        let repo_root = action_root_for_workers.clone();
        let run_context = run_context_for_workers.child();
        async move { Ok(run_one_parallel_task(worker, repo_root, run_context, live_parent).await) }
    })
    .await?;

    let mut results = Vec::with_capacity(n);
    for item in outcome.outcomes {
        match item.result {
            Ok(value) => results.push(value),
            Err(err) => {
                return Err(TinyAgentsError::Graph(format!(
                    "spawn_parallel_agents fan-out: worker {} failed: {err}",
                    item.index
                )));
            }
        }
    }
    if results.len() != n {
        return Err(TinyAgentsError::Graph(format!(
            "spawn_parallel_agents fan-out: expected {n} result(s), got {}",
            results.len()
        )));
    }
    Ok(results)
}

async fn run_one_parallel_task(
    worker: SpawnParallelWorker,
    repo_root: Option<PathBuf>,
    run_context: crate::agent::tinyagents::host::OpenHumanRunContext,
    live_parent: &tinyagents_harness::context::RunContext<
        crate::agent::tinyagents::host::OpenHumanRunContext,
    >,
) -> ParallelAgentResult {
    let SpawnParallelWorker {
        definition,
        prompt,
        task,
        task_id,
        lineage,
        worktree_path,
        workspace_descriptor,
        dispatch_mode: _,
    } = worker;
    let started = std::time::Instant::now();
    tracing::debug!(
        task_id = %task_id,
        agent_id = %definition.id,
        toolkit = task.toolkit.as_deref().unwrap_or(""),
        context_chars = task.context.as_ref().map(|s| s.chars().count()).unwrap_or(0),
        prompt_chars = prompt.chars().count(),
        isolated = worktree_path.is_some(),
        "[spawn_parallel_agents] task_start"
    );
    let worktree_action_dir = worktree_path.clone().or_else(|| {
        workspace_descriptor
            .as_ref()
            .map(|descriptor| descriptor.root.clone())
    });
    let options = SubagentRunOptions {
        skill_filter_override: None,
        toolkit_override: task.toolkit.clone(),
        context: task.context.clone(),
        model_override: None,
        task_id: Some(task_id.clone()),
        thread_id: None,
        run_context,
        worker_thread_id: None,
        initial_history: None,
        checkpoint_dir: None,
        worktree_action_dir,
        workspace_descriptor,
        run_queue: None,
    };
    let run_result =
        run_subagent_with_parent(live_parent, definition.clone(), prompt.clone(), options).await;

    // After the worker finishes, snapshot the worktree's changed files +
    // dirty status so the parent can detect cross-worker overlaps and the UI
    // can surface diff/cleanup actions. Best-effort: a status error degrades
    // to "no changes recorded" rather than failing the task.
    let worktree_str = worktree_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());
    let (changed_files, dirty_status) = match (&worktree_path, &repo_root) {
        (Some(wt), Some(root)) => {
            match tinyagents_harness::workspace::git_worktree_status(root, wt) {
                Ok(st) => {
                    tracing::debug!(
                        task_id = %task_id,
                        worktree = %wt.display(),
                        is_dirty = st.is_dirty,
                        changed = st.changed_files.len(),
                        "[spawn_parallel_agents] worktree_post_run_status"
                    );
                    let files = st
                        .changed_files
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect();
                    (files, Some(st.is_dirty))
                }
                Err(err) => {
                    tracing::warn!(
                        task_id = %task_id,
                        worktree = %wt.display(),
                        error = %err,
                        "[spawn_parallel_agents] worktree_status_failed"
                    );
                    (Vec::new(), None)
                }
            }
        }
        _ => (Vec::new(), None),
    };

    match run_result {
        Ok(outcome) => {
            let emit_lifecycle_effects = outcome.should_emit_lifecycle_effects();
            let (status, success, error, awaiting_question, checkpoint_path) = match &outcome.status
            {
                SubagentRunStatus::Completed => {
                    (ParallelAgentStatus::Completed, true, None, None, None)
                }
                SubagentRunStatus::AwaitingUser {
                    question,
                    checkpoint,
                    ..
                } => (
                    ParallelAgentStatus::AwaitingUser,
                    false,
                    None,
                    Some(question.clone()),
                    checkpoint
                        .as_ref()
                        .map(|path| path.to_string_lossy().to_string()),
                ),
                SubagentRunStatus::Incomplete { reason } => (
                    ParallelAgentStatus::Incomplete,
                    false,
                    Some(reason.clone()),
                    None,
                    None,
                ),
                SubagentRunStatus::Cancelled => (
                    ParallelAgentStatus::Cancelled,
                    false,
                    Some("parallel subagent was cancelled".into()),
                    None,
                    None,
                ),
            };
            tracing::debug!(
                task_id = %outcome.task_id,
                agent_id = %outcome.agent_id,
                elapsed_ms = outcome.elapsed.as_millis() as u64,
                iterations = outcome.iterations,
                output_chars = outcome.output.chars().count(),
                status = ?status,
                "[spawn_parallel_agents] task_outcome"
            );
            ParallelAgentResult {
                task_id: outcome.task_id,
                agent_id: outcome.agent_id,
                lineage,
                success,
                status,
                output: Some(outcome.output),
                error,
                awaiting_question,
                checkpoint_path,
                ownership: task.ownership,
                elapsed_ms: outcome.elapsed.as_millis() as u64,
                iterations: outcome.iterations as u32,
                stale_parent_reads: Vec::new(),
                worktree_path: worktree_str,
                changed_files,
                dirty_status,
                emit_lifecycle_effects,
            }
        }
        Err(err) => {
            tracing::debug!(
                task_id = %task_id,
                agent_id = %definition.id,
                elapsed_ms = started.elapsed().as_millis() as u64,
                error = %err,
                "[spawn_parallel_agents] task_error"
            );
            ParallelAgentResult {
                task_id,
                agent_id: definition.id,
                lineage,
                success: false,
                status: ParallelAgentStatus::Failed,
                output: None,
                error: Some(err.to_string()),
                awaiting_question: None,
                checkpoint_path: None,
                ownership: task.ownership,
                elapsed_ms: started.elapsed().as_millis() as u64,
                iterations: 0,
                stale_parent_reads: Vec::new(),
                worktree_path: worktree_str,
                changed_files,
                dirty_status,
                emit_lifecycle_effects: false,
            }
        }
    }
}
