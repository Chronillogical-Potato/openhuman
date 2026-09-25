---
name: desktop-control
description: Inspect and control native desktop apps when Desktop is enabled in Connections.
---

# Desktop control

Use `tool_search` to discover `desktop_*` tools. Begin with `desktop_list_apps` and `desktop_list_windows`; if the app has no visible window, use `desktop_launch` to activate it. Use the exact `window_id` from `desktop_list_windows` in `desktop_snapshot` and `desktop_goal` to bind both to the same native window; an exact `window` title may also be supplied. Inspect a skeleton `desktop_snapshot` to obtain exact accessible target names, descriptions, or `native_id.value`, and the completion condition. Plan one bounded task, then call `desktop_goal` once: state the app and goal, enumerate allowed mutating operations and exact targets, supply any text in `text_slots` keyed by the target's exact name, description, or `native_id.value`, and give one or more positive `success` predicates using the same target identifier. A missing element in a bounded snapshot does not prove absence. If a target has no accessible name, use its observed native AX identifier when available; otherwise inspect further without broadening the target scope. The module executes and verifies multiple actions inside this one call. Read its `verified`, `stop`, and final observation before reporting completion.

If a ref is stale, take a fresh snapshot. If Accessibility is denied, ask the user to grant it to the process running OpenHuman. Desktop goals have action, model-call, and elapsed-time limits. With desktop approvals off (the default), the module runs in-scope actions without stopping for per-action approval. If the operator enables desktop approvals and the module returns a pending action, wait for their decision in Connections before using `desktop_continue_goal`. If `verified` is false, use the returned evidence to replan or explain the blocker; do not replay an uncertain action. Never claim success from a timeout or a model decision alone.
