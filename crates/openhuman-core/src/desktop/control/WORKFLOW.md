---
name: desktop-control
description: Inspect and control native desktop apps when Desktop is enabled in Connections.
---

# Desktop control

Use `tool_search` to discover `desktop_*` tools. Begin with `desktop_list_apps` and `desktop_list_windows`; if the app has no visible window, use `desktop_launch` to activate it. Take a skeleton `desktop_snapshot`, then use `desktop_find` or a deeper snapshot to select a snapshot-qualified element ref. After an action, observe the app again and verify the visible result.

If a ref is stale, take a fresh snapshot. If Accessibility is denied, ask the user to grant it to the process running OpenHuman. Desktop goals have small action and model-call limits. With desktop approvals off (the default), the core continues a confirmation stop through the module's one-use handle; the module reobserves and validates the target before acting. If the operator enables desktop approvals, wait for their decision in Connections before using `desktop_continue_goal`. Never claim an action succeeded from a timeout or a model decision alone.
