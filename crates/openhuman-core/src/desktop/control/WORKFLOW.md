---
name: desktop-control
description: Inspect and control native desktop apps when Desktop is enabled in Connections.
---

# Desktop control

Use `tool_search` to discover `desktop_*` tools. Begin with `desktop_list_apps` and `desktop_list_windows`; if the app has no visible window, use `desktop_launch` to activate it. Use the exact `window_id` from `desktop_list_windows` in `desktop_snapshot` and `desktop_goal` to bind both to the same native window; an exact `window` title may also be supplied. Inspect a skeleton `desktop_snapshot` to obtain exact accessible target names, descriptions, or `native_id.value`, and the completion condition. Plan one bounded task, then call `desktop_goal` once: state the app and goal, enumerate allowed mutating operations and exact targets, supply any text in `text_slots` keyed by the target's exact name, description, or `native_id.value`, and give one or more positive `success` predicates using the same target identifier. A missing element in a bounded snapshot does not prove absence. If a target has no accessible name, use its observed native AX identifier when available; otherwise inspect further without broadening the target scope. The module executes and verifies multiple actions inside this one call. Read its `verified`, `stop`, and final observation before reporting completion.

## Prepare a task from the user's words

Choose the exact running app label from `desktop_list_apps`, including any direction marks in its accessible name. Treat text the user asks to send or type as a fixed value: copy it verbatim into one `text_slots` entry, preserving capitalization, punctuation, and spaces. The slot key must be one of the exact `allowed_targets` from the snapshot. Jev chooses actions and observed targets; it never writes or revises the prepared text. Keep the operation and target lists to those needed for this task.

For a chat send, first inspect the active conversation header, the message composer, and the send control. A contact row in the chat list does not prove that its conversation is open. Scope the goal to the intended recipient's conversation, allow only the observed row, composer, and send targets, and require both the active message-list header with `name_present` and the outgoing message with `name_contains`. Set `fragment` to the visible outgoing-message prefix containing the exact prepared text, and `within` to that active message-list header. If the app's accessibility labels do not expose those facts, inspect further and stop without sending. An unverified send must never be repeated merely because the first result has not appeared yet.

For Spotify playback, inspect the current player and playlist controls. A running Spotify process or Jev `DONE` answer is insufficient; use a fresh accessible Pause control for the chosen track as the success condition. If that relationship is not visible, narrow the task and report the verification limit.

If a ref is stale, take a fresh snapshot. If Accessibility is denied, ask the user to grant it to the process running OpenHuman. Desktop goals have action, model-call, and elapsed-time limits. With desktop approvals off (the default), the module runs in-scope actions without stopping for per-action approval. If the operator enables desktop approvals and the module returns a pending action, wait for their decision in Connections before using `desktop_continue_goal`. If `verified` is false, use the returned evidence to replan or explain the blocker; do not replay an uncertain action. Never claim success from a timeout or a model decision alone.
