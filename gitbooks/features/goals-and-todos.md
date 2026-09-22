---
description: Durable user goals and the agent's native TinyAgents work state.
icon: target
---

# Goals & Todos

## Long-term goals

OpenHuman keeps a short, human-readable list of durable user objectives in
`MEMORY_GOALS.md`. The Intelligence → Goals panel supports adding, editing, and
deleting entries, while the goals reflection agent can make small changes based
on recent memory and conversations.

The list is deliberately bounded to keep it useful in prompts. Each entry has a
stable short id so edits do not depend on ordering. The corresponding RPC
surface is `openhuman.memory_goals_*`.

## Agent work state

While it works on a multi-step request the agent keeps a session todo list,
the same shape Claude Code and Codex use: one `todo` tool call writes the whole
list (`content` + `pending` / `in_progress` / `completed`), scoped to the agent
session and held in memory for the life of the process. Thread goals are the
per-thread completion contract (`goal_set` / `goal_get` / `goal_complete`).

Neither is a kanban board. There is no per-thread task board, no card CRUD,
no approval gate, and no `thread_goals`, `todos`, or `threads_task_board` RPC
endpoint. Conversation threads remain the chat/session container.

## See also

- [Memory Tree](obsidian-wiki/memory-tree.md): what goal reflection reads from.
