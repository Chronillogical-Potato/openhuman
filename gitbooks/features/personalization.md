---
description: >-
  How OpenHuman learns your preferences from everyday use: learnings in memory,
  the memory pack recalled for each turn, and your persona files.
icon: brain
---

# Personalization & Self-Learning

OpenHuman gets to know you through [memory](memory.md), not a settings form. Three things shape how it behaves toward you, and all three are visible and editable.

## Learnings

A **learning** is one durable statement: a preference, fact, procedure or correction ("prefers pnpm", "never schedule meetings before 10"). The agent saves one with its `memory` tool (`learn` action) when you state or correct something worth keeping, and you can add your own on **Connections → Memory → Learnings**. Each learning records where it came from (workspace, thread, agent, the tool call). Delete any learning there, or ask the agent to forget it.

Learnings live in your selected memory engine, so memory has to be on (see [Engines](memory.md#engines)). Secrets and personal identifiers are scrubbed before storing.

## The memory pack

Before every turn, OpenHuman recalls a short, token-budgeted *memory pack* from memory: relevant learnings (and beliefs the engine has built from them), documents from your brain, this agent's earlier conversations and, briefly, other agents' turns. The pack is added to that turn's model request only and is never written into the chat, so the agent defaults to your preferences without being asked and the prompt cache is unaffected. Preview it on **Connections → Memory → Ask**. See [How the agent uses memory](memory.md#how-the-agent-uses-memory).

## Persona files

How the agent presents itself (`SOUL.md`, `IDENTITY.md`, `ROLE.md` in your workspace) is separate from memory. These are plain Markdown files you edit directly.

## See also

- [Memory](memory.md), the engines, sources and tabs behind all of this.
- [Memory tools](native-tools/memory-tools.md), how the agent recalls and learns.
- [Goals & To-dos](goals-and-todos.md), the agent's in-session work tracking.
- [Cron & Scheduling](native-tools/cron.md), the scheduled runs that keep working your workspace between turns.
