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

Turn-scoped goals and todos are internal TinyAgents capabilities. TinyAgents
owns their types, lifecycle, persistence, budgets, claims, and run records;
OpenHuman supplies runtime and tool adapters so the orchestrator can use them
while it works.

These internals are not presented as a separate kanban board and do not expose
`thread_goals`, `todos`, or `threads_task_board` RPC endpoints. Conversation
threads remain the chat/session container and are independent of this agent
work state.

## See also

- [Memory Tree](obsidian-wiki/memory-tree.md): what goal reflection reads from.
