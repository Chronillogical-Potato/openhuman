---
description: Tools the agent uses to plan, delegate and ask for help.
icon: sitemap
---

# Agent coordination

Besides doing the work, the agent has tools for organizing it: planning multi-step jobs, delegating to specialists, spawning sub-agents, and pausing to ask you when something is genuinely unclear.

## Tools in this family

| Tool                                | What it does                                                                                                           |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `todo`                              | Rewrites the session to-do list across a long task. The chat shows it as a checklist that ticks off as work progresses. |
| `spawn_subagent`                    | Delegates to a reusable async specialist. It creates a fresh worker only when the work is incompatible or you ask.     |
| `spawn_async_subagent`              | A lower-level reusable async delegation tool with the same durable session identity.                                   |
| `steer_subagent`, `wait_subagent`   | Message or collect a running worker by durable `subagent_session_id` or transient `task_id`.                           |
| `list_subagents`, `close_subagent`  | Inspect the reusable workers for the parent thread, or retire one.                                                     |
| `spawn_worker_thread`               | Explicit background work tracked as a separate worker thread.                                                          |
| `delegate`                          | Hands a task to a specialist, such as an archetype with different prompts, tools or permissions.                       |
| `spawn_parallel_agents`             | Fans one task out to several specialists at once and merges what they return.                                          |
| `use_skill`                         | Loads an inline skill's playbook and tools (coding, web3, system, scheduling, docs, mcp) so the agent can call them.   |
| `ask_user_clarification`            | Pauses and asks you a precise question instead of guessing.                                                            |
| `plan_exit`                         | Leaves the planning phase and starts executing.                                                                        |

`spawn_subagent` and archetype delegation calls accept an optional `model` field to pin an exact model for one call. If you leave it out, the harness uses per-agent pins from config when present, and otherwise falls back to normal model-routing hints. Model, sandbox mode, parent thread, action root and task key all affect whether a sub-agent can be reused, so materially different work gets its own worker.

Reusable delegation returns a transient `task_id` and a durable `subagent_session_id`. Use the durable id for follow-ups in later turns. Pass `fresh: true` only when you or the task needs a clean worker. Pass `blocking: true` only when the parent must wait for the child's result.

## Why these are tools

Long tasks fall apart when an agent tries to hold everything in one head. Splitting work with to-dos and sub-agents helps in three ways:

- Each sub-agent keeps useful local context for the same job instead of being respawned every turn.
- The main thread keeps a high-level view of progress.
- A failure in one branch does not spoil the rest.

Asking for clarification is a tool on purpose. It makes "I should ask the user" a visible decision the agent can be steered toward, instead of something that only happens by chance.

## See also

- [Coder](coder.md): the coding tools, most of them loaded through the `coding` skill.
- [Cron and scheduling](cron.md): how background agent runs are scheduled.
