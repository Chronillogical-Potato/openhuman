---
description: >-
  One page for everything the agent plugs into: apps, channels, MCP servers,
  skills, memory, tools, the computer, the wallet, and every API key.
icon: plug
---

# Connections

**Connections** is the hub. Almost everything you would call a setting that changes what the agent *can do*, rather than how it looks, lives here, split into two groups in the left rail.

Most of the older addresses redirect here, so a bookmark to `/skills` or `/channels` still lands in the right place.

## Integrations

| Tab | What it holds |
| --- | --- |
| **Apps** | The OAuth connectors. Pick a toolkit, authorize it in a browser window, and choose its scope (read, write or admin) and which triggers may fire. See [Third-party Integrations](integrations/README.md). |
| **Messaging** | The channels the agent talks to you on: Telegram, Discord, the in-app web chat, Lark/Feishu, DingTalk, iMessage, email over IMAP and SMTP, and 元宝. See [Messaging Channels](channels.md). |
| **MCP** | Model Context Protocol servers, in four views: your servers, client config, the raw `mcp.json` (which is the only place a server is actually added), and a browsable registry. Includes a playground for calling one tool by hand. |
| **Skills** | Installed skill bundles, a registry to browse and install from, and a runner. See [MCP Servers & Skills](integrations/mcp-and-skills.md). |
| **Brain** | [Memory](memory.md): the engine, ask, explorer, learnings, conversations, the shared brain, background jobs and recall settings. |
| **Agent tools** | Which tools the agent may use, and the per-tool policy. |
| **Computer** | Desktop and browser control, the decision model, and the planner. See [Browser & Computer Control](native-tools/browser-and-computer.md). |
| **Wallet** | Balances, send and receive, and the recovery phrase. See [Wallet](wallet.md). |

## API keys

The second group is one tab per engine family, and each one is the same shape: use the managed TinyHumans route, or paste your own key.

| Tab | Covers |
| --- | --- |
| **LLM** | Model providers: the managed route, your own keys for 26 built-in providers, and local servers (Ollama, LM Studio, MLX, any OpenAI-compatible endpoint). See [Automatic Model Routing](model-routing/README.md). |
| **Apps key** | Your own connector-platform key, if you would rather not use the managed route. |
| **Voice** | Speech-to-text and text-to-speech providers, per-workload routing, and the dictation hotkey. |
| **Embeddings** | The embedding provider behind memory and tool search. |
| **Search** | Web-search providers. Two have a managed route; the rest are bring-your-own-key. See [Web Search](native-tools/web-search.md). |
| **Usage** | What you have spent, by day and by model, plus the background-activity log. See [Billing, Cost & Usage](billing-and-usage.md). |

## Why one page

These used to be a dozen separate settings screens, and the split never held: connecting Gmail is an integration, a memory source, a trigger source and a set of agent tools at once, and so is connecting GitHub. Keeping them on one page with one rail means the thing you are setting up stays in front of you while you set up the parts of it that live in other systems.

Settings that change how the app *behaves* rather than what it reaches, appearance, personality, security, keychain, autonomy, stay in [Settings](settings.md).

## See also

- [Getting Started](../overview/getting-started.md): the first-run path through this page.
- [Memory](memory.md), [Messaging Channels](channels.md), [Third-party Integrations](integrations/README.md): the three tabs with the most behind them.
- [Privacy & Security](privacy-and-security.md): what each connection means for what leaves your machine.
