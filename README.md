<h1 align="center">OpenHuman</h1>

<p align="center">
 <img src="./demo.gif" alt="A walkthrough of the OpenHuman desktop app" />
</p>

<p align="center">
 <strong>The open-source agent harness that runs fast, runs cheap, and runs hundreds of agents in one process.</strong><br/>
 A Rust core you can use as a desktop app or embed as a library.
</p>

<p align="center">
	<a href="https://trendshift.io/repositories/23680" target="_blank">
		<img src="https://trendshift.io/api/badge/repositories/23680" alt="tinyhumansai%2Fopenhuman | Trendshift" width="250" height="55"/>
	</a>
	<a href="https://www.producthunt.com/products/openhuman?embed=true&amp;utm_source=badge-top-post-badge&amp;utm_medium=badge&amp;utm_campaign=badge-openhuman" target="_blank" rel="noopener noreferrer">
		<img alt="OpenHuman on Product Hunt" width="250" height="54" src="https://api.producthunt.com/widgets/embed-image/v1/top-post-badge.svg?post_id=1136902&amp;theme=light&amp;period=daily&amp;t=1778916022823">
	</a>
	<a href="https://www.producthunt.com/products/openhuman?embed=true&amp;utm_source=badge-top-post-badge&amp;utm_medium=badge&amp;utm_campaign=badge-openhuman" target="_blank" rel="noopener noreferrer">
		<img alt="OpenHuman on Product Hunt" width="250" height="54" src="https://api.producthunt.com/widgets/embed-image/v1/top-post-badge.svg?post_id=1136902&amp;theme=light&amp;period=weekly&amp;t=1779351403565">
	</a>
</p>

<p align="center">
 <img src="https://img.shields.io/badge/status-early%20beta-orange" alt="Early Beta" />
 <a href="https://github.com/tinyhumansai/openhuman/releases/latest"><img src="https://img.shields.io/github/v/release/tinyhumansai/openhuman?label=latest" alt="Latest Release" /></a>
 <a href="https://github.com/tinyhumansai/openhuman/stargazers"><img src="https://img.shields.io/github/stars/tinyhumansai/openhuman?style=flat" alt="GitHub Stars" /></a>
 <a href="./LICENSE"><img src="https://img.shields.io/github/license/tinyhumansai/openhuman" alt="License" /></a>
 <a href="https://github.com/tinyhumansai/openhuman-benchmarks"><img src="https://img.shields.io/badge/benchmarks-public-brightgreen" alt="Public benchmarks" /></a>
</p>

<p align="center">
 <a href="https://tinyhumans.gitbook.io/openhuman/">Docs</a> ·
 <a href="./gitbooks/developing/quickstart.md">Rust quickstart</a> ·
 <a href="https://tinyhumansai.github.io/openhuman-benchmarks/">Benchmarks</a> ·
 <a href="https://github.com/tinyhumansai/openhuman/discussions">Discussions</a> ·
 <a href="https://guild.tinyhumans.ai/">Discord</a> ·
 <a href="https://www.reddit.com/r/tinyhumansai/">Reddit</a> ·
 <a href="https://x.com/intent/follow?screen_name=tinyhumansai">X</a> ·
 <a href="https://x.com/intent/follow?screen_name=senamakel">@senamakel (creator)</a>
</p>

<p align="center">
  <a href="./README.md">English</a> | <a href="./docs/README.ar.md">العربية</a> | <a href="./docs/README.zh-CN.md">简体中文</a> | <a href="./docs/README.ja-JP.md">日本語</a> | <a href="./docs/README.ko.md">한국어</a> | <a href="./docs/README.de.md">Deutsch</a> | <a href="./docs/README.tr.md">Türkçe</a> | <a href="./docs/README.ur-pk.md">اردو</a>
</p>

> **Early beta.** OpenHuman is under active development, so expect rough edges. The checklist for leaving beta is public: [#6047](https://github.com/tinyhumansai/openhuman/issues/6047).


---

## Why OpenHuman

Most agent harnesses run one Node or Python process per agent, send a large system prompt on every call, and get more expensive with every turn. OpenHuman is a Rust core that does the same work with less of everything. It is also the only feature-rich open-source harness built to run large fleets of agents.

<table>
<tr>
<td width="50%" valign="top">

### Fast

The fastest wall time in the latest public SWE-bench run: **20 s** per solved task at the median, against 28 to 62 s for the other six harnesses. The first token arrives in 1.2 s, and an agent turn starts cold in 102 ms.

</td>
<td width="50%" valign="top">

### Cheap

**2.6x fewer tokens** per solved task than the median harness, and the lowest total spend of the run. It sends the smallest prompt of the field, and [token compression](./gitbooks/features/token-compression.md) shrinks tool output before the model sees it.

</td>
</tr>
<tr>
<td width="50%" valign="top">

### Efficient at scale

**68 MB** of memory and **1.3 CPU-seconds** per solved task, about 8x less than the median harness. Inside one process each extra agent costs about 1.8 MiB, so [500 agents](./gitbooks/developing/performance.md) fit in 1.4 GiB.

</td>
<td width="50%" valign="top">

### Built for developers

OpenHuman is a library first. Call an agent as a function from your own Rust code, or run a fleet of feature-rich agents (memory, tools, integrations, sandboxes) inside one process. Every engine is chosen by config and every capability is a Cargo feature.

</td>
</tr>
</table>

---

## Benchmarks

<p align="center">
 <picture>
  <source media="(prefers-color-scheme: dark)" srcset="./gitbooks/.gitbook/assets/benchmarks/swe-x86-1-dark.png" />
  <source media="(prefers-color-scheme: light)" srcset="./gitbooks/.gitbook/assets/benchmarks/swe-x86-1.png" />
  <img alt="SWE-bench Verified run swe-x86-1: OpenHuman against six other harnesses on ten panels" src="./gitbooks/.gitbook/assets/benchmarks/swe-x86-1.png" />
 </picture>
</p>

The numbers come from [openhuman-benchmarks](https://github.com/tinyhumansai/openhuman-benchmarks), a public repository anyone can rerun. It puts OpenHuman, Claude Code, Codex, OpenCode, OpenClaw, Hermes and DeepSeek Harness on the same SWE-bench Verified tasks with the same model, the same key and the same container limits, and it measures every call from the wire through one metering proxy. The latest run, [`swe-x86-1`](https://github.com/tinyhumansai/openhuman-benchmarks/blob/main/results/swe-x86-1/summary.md), is ten tasks: a smoke test, not a ranking.

| Per solved task | OpenHuman | Median of the other six |
| --- | --- | --- |
| Wall time | **19.8 s** | 46.5 s |
| Tokens | **167k** | 435k |
| Peak memory | **68 MB** | 523 MB |
| CPU time | **1.3 s** | 10.4 s |
| Tasks resolved | 7 / 10 | 10 / 10 |

---

## For users

You want an assistant that knows your work, acts in the apps you already use, and does not send everything to someone else's cloud. That is what the desktop app is for. Download it for Windows, macOS or Linux from [tinyhumans.ai/openhuman](https://tinyhumans.ai/openhuman?utm_source=github&utm_medium=readme) or [GitHub Releases](https://github.com/tinyhumansai/openhuman/releases/latest) (terminal installs are in [INSTALL.md](./INSTALL.md)).

| If you want to | OpenHuman gives you |
| --- | --- |
| Stop re-explaining context | A [memory](./gitbooks/features/memory.md) of your files, repos and apps, recalled before every reply, with citations |
| Get real work done, not just answers | [Native tools](./gitbooks/features/native-tools/README.md) for search, browsing, code, documents, voice and media, plus an [orchestrator](./gitbooks/features/orchestration.md) for big jobs |
| Work inside your apps | [119 one-click integrations](./gitbooks/features/integrations/README.md), [MCP servers and skills](./gitbooks/features/integrations/mcp-and-skills.md) |
| Reach it from anywhere | [14 messaging channels](./gitbooks/features/channels.md), including Telegram, Discord, iMessage and email |
| Automate the boring parts | [Workflows](./gitbooks/features/workflows.md) the agent drafts for you to review and save |
| Choose your model and your bill | [Local models or your own key](./gitbooks/features/model-routing/local-and-byok-models.md), with [automatic routing](./gitbooks/features/model-routing/README.md) |
| Keep your data yours | [Privacy Mode](./gitbooks/features/privacy-mode.md), the [OS keyring](./gitbooks/features/os-keyring-and-secret-storage.md) and an [approval gate](./gitbooks/features/approval-gate.md) |

New to it? Start with [Getting started](./gitbooks/overview/getting-started.md) or one of the [guides](./gitbooks/guides/README.md).

---

## For developers

OpenHuman is a library-first harness. You add it to an existing Rust codebase and call an agent the way you call any other function: no sidecar process, no daemon, no RPC hop. When one agent is not enough, the same runtime holds hundreds of them, each with its own model, tools, memory and sandbox, at a fraction of the memory and tokens other harnesses spend.

```rust
use openhuman_embed::{Access, Harness, Provider, Workspace};

let agent = Harness::builder()
    .provider(Provider::openai_compatible("https://api.openai.com/v1", "sk-...").model("gpt-5"))
    .workspace(Workspace::Ephemeral)
    .access(Access::readonly())
    .build()
    .await?;

let reply = agent.run("Summarize what you can see in this directory.").await?;
println!("{}", reply.reply);
```

Start with the [Rust quickstart](./gitbooks/developing/quickstart.md), then the [embedding reference](./gitbooks/developing/embedding.md). Architecture, engines, modules and performance are in the [developer docs](./gitbooks/developing/README.md).

---

## How it compares

High-level comparison (products change, so verify against each project).

|                         | Claude Cowork     | OpenClaw          | Hermes Agent      | OpenHuman                                                          |
| ----------------------- | ----------------- | ----------------- | ----------------- | ------------------------------------------------------------------ |
| **Open source**         | 🚫 Proprietary    | ✅ MIT            | ✅ MIT            | ✅ GPL-3.0                                                         |
| **Simple to start**     | ✅ Desktop + CLI  | ⚠️ Terminal-first | ⚠️ Terminal-first | ✅ Clean UI, minutes                                               |
| **Cost**                | ⚠️ Sub + add-ons  | ⚠️ BYO models     | ⚠️ BYO models     | ✅ 2.6x fewer tokens, one subscription                             |
| **Agent fleets**        | 🚫 One session    | ⚠️ Process per agent | ⚠️ Process per agent | 🚀 Hundreds of agents in one process, ~1.8 MiB each          |
| **Embeddable library**  | ⚠️ SDK over a CLI | 🚫 None           | ⚠️ Python package | 🚀 Typed Rust API                                                  |
| **Memory**              | ✅ Chat-scoped    | ⚠️ Plugin-reliant | ✅ Self-learning  | 🚀 Pluggable engine, recall before every turn, citations          |
| **Integrations**        | ⚠️ Few connectors | ⚠️ BYO            | ⚠️ BYO            | 🚀 119 managed-auth OAuth apps, MCP, skills                        |
| **Source sync**         | 🚫 None           | 🚫 None           | 🚫 None           | ✅ Folders, repos, feeds and apps synced into memory               |
| **Orchestration**       | ⚠️ Sub-tasks      | ⚠️ Single loop    | ⚠️ Single loop    | 🚀 Agent graphs with checkpoints                                   |
| **Workflows**           | 🚫 None           | ⚠️ Scripts        | ⚠️ Scripts        | 🚀 Visual, durable, agent-drafted, approval-gated                  |
| **Messaging channels**  | 🚫 None           | ✅ Many           | ✅ Several        | ✅ 14 in the shipped build, including native email                 |
| **Local-only mode**     | 🚫 Cloud-only     | ⚠️ BYO local      | ⚠️ BYO local      | ✅ One-switch Privacy Mode                                         |
| **Observability**       | 🚫 Opaque         | ⚠️ Logs           | ⚠️ Logs           | ✅ Replayable run journals, per-call cost                          |
| **Public benchmarks**   | 🚫 None           | 🚫 None           | 🚫 None           | ✅ [Reproducible, wire-metered](https://github.com/tinyhumansai/openhuman-benchmarks) |
| **Model routing**       | 🚫 Single vendor  | ⚠️ Manual         | ⚠️ Manual         | ✅ Built-in                                                        |
| **Native tools**        | ✅ Code-focused   | ✅ Code-focused   | ✅ Code-focused   | ✅ Code, search, scraper, browser, voice, media generation         |

---

## Contributing

New here? [`CONTRIBUTING.md`](./CONTRIBUTING.md) covers the fork and PR workflow and the local checks, and [`CONTRIBUTING-BEGINNERS.md`](./docs/CONTRIBUTING-BEGINNERS.md#optional--let-an-ai-coding-agent-guide-you) has a copy-paste prompt that lets an AI coding agent walk you through your first change.

The short path:

1. Install Git, Node.js 24+, pnpm 10.10.0, Rust 1.96.1 (with `rustfmt` and `clippy`), CMake, Ninja, ripgrep, and your platform's desktop build prerequisites.
2. Fork and clone, then run `git submodule update --init --recursive` before `pnpm install` so the vendored crates under `vendor/` resolve.
3. Run `pnpm dev` for UI work or `pnpm dev:app` for the desktop shell, and check your change with `pnpm typecheck`, `pnpm format:check` and `cargo check --manifest-path Cargo.toml` before opening a PR.

[Getting set up](./gitbooks/developing/getting-set-up.md) has the full environment guide, [`AGENTS.md`](./AGENTS.md) has the conventions the codebase follows, and the [crates overview](./crates/README.md) maps the workspace. Much of OpenHuman lives in its own open repositories ([tinyagents](https://github.com/tinyhumansai/tinyagents), [tinyflows](https://github.com/tinyhumansai/tinyflows), [tinymemory](https://github.com/tinyhumansai/tinymemory) and more under [`vendor/`](./vendor)), and contributions there are just as welcome.

Contributors get free merch and special access on [Discord](https://guild.tinyhumans.ai/).


## Star history

<p align="center">
 <a href="https://www.star-history.com/#tinyhumansai/openhuman&type=date&legend=top-left">
 <picture>
 <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=tinyhumansai/openhuman&type=date&theme=dark&legend=top-left" />
 <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=tinyhumansai/openhuman&type=date&legend=top-left" />
 <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=tinyhumansai/openhuman&type=date&legend=top-left" />
 </picture>
 </a>
</p>

## Contributors

<a href="https://github.com/tinyhumansai/openhuman/graphs/contributors">
 <img src="https://contrib.rocks/image?repo=tinyhumansai/openhuman" alt="OpenHuman contributors" />
</a>
