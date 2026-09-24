---
name: desktop-control
description: Inspect and control native desktop apps when Desktop is enabled in Connections.
---

# Desktop control

Use `tool_search` to discover `desktop_*` tools. Begin with `desktop_list_apps` and `desktop_snapshot` (skeleton first); use `desktop_find` or a deeper snapshot to select a snapshot-qualified element ref. After an action, observe the app again and verify the visible result.

If a ref is stale, take a fresh snapshot. If Accessibility is denied, ask the user to grant it to the process running OpenHuman. Desktop goals have small action and model-call limits. A confirmation stop names the proposed operation and target; wait for the user to approve it in Connections before continuing with the `confirmation_id`. Never claim an action succeeded from a timeout or a model decision alone.
