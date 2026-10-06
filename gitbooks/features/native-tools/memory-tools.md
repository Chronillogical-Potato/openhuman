---
description: How the agent recalls, fetches, learns and forgets with its single memory tool.
icon: brain
---

# Memory Tools

[Memory](../memory.md) is OpenHuman's knowledge base. The agent talks to it through one tool, `memory`, whose `action` is one of:

| Action | What it does |
| --- | --- |
| `recall` | Ask a question; returns an answer and the citations it rests on. |
| `fetch` | Raw search (hybrid) over stored items, optionally filtered by metadata. Returns hits. |
| `learn` | Save one durable learning (a preference, fact, procedure or correction). |
| `forget` | Remove items by id. |

The tool is only registered when a memory engine is usable. With none selected, memory is off and the agent never sees it. Each learning is shared with every agent under the same memory root and tagged with the workspace, thread, agent and tool call that produced it.

## The tool and the per-turn pack

Every turn already arrives with a small [memory pack](../memory.md#how-the-agent-uses-memory) (`<memory-context>`): the learnings, documents and history most relevant to what you just said. The tool is for going further than the pack: asking a question the pack does not cover ("what do I know about the Stripe webhook?"), searching with metadata filters, or saving something worth remembering next time.

External MCP clients get the same abilities from OpenHuman's [MCP server](../../developing/mcp-server.md) as `memory.recall`, `memory.fetch`, `memory.list`, `memory.learn` and `memory.forget`.

## See also

- [Memory](../memory.md)
