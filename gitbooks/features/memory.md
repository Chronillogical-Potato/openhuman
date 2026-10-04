---
description: >-
  Memory v2: a pluggable engine that stores your documents, conversations and
  learnings, answers questions with citations, and keeps a brief for new chats.
icon: brain
---

# Memory

OpenHuman's memory lets the agent remember you across chats: what you are working on, what you prefer, what is in your documents. Memory is a feature of the app, not a hidden database. You pick who stores it, see what is stored, and delete anything.

Open it from **Connections → Memory**. It has seven tabs (chips): **Engine**, **Ask**, **Explorer**, **Learnings**, **Conversations**, **Documents** and **Context**. The old `/brain` and `/settings/memory-engine` addresses redirect there.

## Engines

An engine stores your memory and answers questions about it. There are two:

| Engine | What it is | Needs |
| --- | --- | --- |
| **TinyHumans** | Hosted CortexDB run by TinyHumans | You are signed in |
| **CortexDB** | Your own CortexDB | An endpoint and an API key (kept in the OS keychain) |

Pick one on the **Engine** tab. If you are signed out and have no CortexDB key, memory is **off**: the agent has no memory tool, nothing is stored, and the Memory page tells you why.

## What the engine does

- **Recall** answers a question in plain language and cites the stored items it relied on. The engine implements it.
- **Fetch** is raw search over stored items with metadata filters (folder, repo, thread, source, time window). Both launch engines support hybrid search only.
- **Store** takes three kinds of items: documents, conversations and learnings.

## What gets stored

### Documents

Add a **source** on the **Documents** tab. A source is one of:

| Kind | Target |
| --- | --- |
| `folder` | A folder on your computer |
| `file` | A single file |
| `link` | A web page |
| `github` | A repository (`owner/repo`) |
| `rss` | A feed URL |
| `composio` | A connected integration (Composio toolkit) |

Sources sync when you press sync and on a schedule you set per source. An unchanged file that syncs again is not stored twice. Removing a source can also forget the items it produced.

### Conversations

After a few committed turns in a thread, or once a thread has been idle for a while, OpenHuman stores the recent turns as one conversation item. Defaults are every 4 turns or 120 seconds idle; change them on the **Conversations** tab, which also lets you turn it off. Tool calls are stored by name and id only, never their arguments. Anything still buffered is flushed when you quit the app, within a short time limit.

### Learnings

A learning is one durable statement: a preference, fact, procedure or correction. The agent saves them with its `memory` tool (`learn` action), and you can add or delete them on the **Learnings** tab.

Everything is scrubbed for secrets and personal identifiers before it leaves your machine.

### Past conversations

Chats from before automatic saving was turned on are not in memory until you sync them. The **Conversations** tab shows how many chats and turns are still unsynced; **Sync past conversations** uploads them (after you confirm) in the same form as new conversations, without tool arguments. You can close the page while it runs, and syncing again later only sends what is new.

## Exploring what memory holds

The **Explorer** tab shows everything your memory holds, grouped by one property at a time: type, memory node, source, workspace, folder, file, language, repository, link, thread, agent, tool or tag. Each value shows how many items carry it. Click one to narrow to those items, then group again by another property; the breadcrumb at the top takes you back up. The items at each step are listed below, and **Open** shows one in full with all its details and a **Forget** button.

On a very large memory the counts may cover only the items scanned so far; the tab says so when that happens.

## Each agent's own memory

Memory is shared where it should be and private where it should be:

- **Shared (root).** What the main assistant learns, your synced documents, and your imported and past conversations. Every agent can read it.
- **One node per agent.** Every other agent (a specialist, a team member, an agent you run through OpenHuman as a library) keeps its own learnings, documents and conversations in its own node. A sub-agent's node sits under the agent that started it, and a team member's under its team, so teammates share the team's node.

An agent reads its own memory and everything shared above it, never another agent's. When an agent learns something everyone should know, it stores it as shared (`learn` with `share: true`). Each agent also gets its own `context.md`, built from what it can read.

In `config.toml`, `[memory] root_agents` lists the agents that use the shared node (the main assistant by default), and `[memory.agents.<id>]` can pin an agent to a particular node (`namespace = "project:q4"`), stop it reading shared memory (`inherit = false`) or switch its `context.md` off (`context = false`). A document source can be stored at an agent's node with its `namespace`. In the **Explorer**, group by **Memory node** to see what each agent holds.

## The agent's memory tool

The agent has one tool, `memory`, with four actions: `recall`, `fetch`, `learn` and `forget`. OpenHuman's own [MCP server](../developing/mcp-server.md) exposes the same abilities to other apps as `memory.recall`, `memory.fetch`, `memory.list`, `memory.learn` and `memory.forget`.

## context.md

Every six hours (and on demand from the **Context** tab) OpenHuman asks the engine for a short brief about you: who you are, active work, preferences and standing instructions, recent important events, plus your learnings. It is saved as `<workspace>/memory/context.md`, trimmed to a token budget (default 2000), and placed at the start of **new** chats only. Every other agent gets its own brief from its own memory plus what is shared with it (`<workspace>/memory/context/agent-<id>/context.md`); pick the agent's memory node on the **Context** tab to read or regenerate it. A chat you resume keeps the prompt it started with, so it does not see a newer brief. Interval, budget and an on/off switch are on the **Context** tab.

## Importing your previous memory

If OpenHuman finds memory from the earlier (v1) version, the Memory page offers a one-time import. It uploads that data to the engine you selected, so it only starts after you explicitly consent. It resumes if interrupted.

## Removed in v2

The memory tree, graph view, goals list, people and contacts, learning profile, `MEMORY.md` and `PROFILE.md`, the Obsidian vault, the local TinyCortex engine and the Supermemory, Mem0, Cognee and agentMemory engines are gone, as is engine migration. See the [spec](https://github.com/tinyhumansai/openhuman/blob/main/docs/specs/memory-v2.md).

## See also

- [Memory architecture](../developing/architecture/memory.md)
- [Pluggable engines](../developing/engines.md)
- [Privacy and security](privacy-and-security.md)
