---
description: How the agent recalls, fetches, learns and forgets with its single memory tool.
icon: brain
---

# Memory tools

[Memory](../memory.md) is OpenHuman's knowledge base. The agent uses it through one tool, `memory`, whose `action` is one of these:

| Action | What it does |
| --- | --- |
| `recall` | Ask a question. Returns an answer and the citations it rests on. |
| `fetch` | Raw hybrid search over stored items, optionally filtered by metadata. Returns hits. |
| `learn` | Save one durable learning: a preference, fact, procedure or correction. |
| `forget` | Remove items by id. |

The tool is registered only when a memory engine is usable. With none selected, memory is off and the agent never sees the tool. Every learning is shared with all agents under the same memory root. It is tagged with the workspace, thread, agent and tool call that produced it.

## The tool and the per-turn pack

Every turn already arrives with a small [memory pack](../memory.md#how-the-agent-uses-memory) (`<memory-context>`). It holds the learnings, documents and history most relevant to what you just said.

Use the tool to go further than the pack. The agent can ask a question the pack does not cover ("what do I know about the Stripe webhook?"), search with metadata filters, or save something worth remembering next time.

External MCP clients get the same abilities from OpenHuman's [MCP server](../../developing/mcp-server.md) as `memory.recall`, `memory.fetch`, `memory.list`, `memory.learn` and `memory.forget`.

## See also

- [Memory](../memory.md)
