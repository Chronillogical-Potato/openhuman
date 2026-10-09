<h1 align="center">OpenHuman</h1>

<p align="center">
 <strong>The fastest, cheapest, most efficient open-source agent harness. Run more than 500 agents on a $10 VPS.</strong><br/>
 A desktop app for people. A Rust library for developers.
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
 <img src="./demo.gif" alt="A walkthrough of the OpenHuman desktop app" />
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
  <a href="./README.md">English</a> | <a href="./docs/README.ar.md">العربية</a> | <a href="./docs/README.zh-CN.md">简体中文</a> | <a href="./docs/README.ja-JP.md">日本語</a> | <a href="./docs/README.ko.md">한국어</a> | <a href="./docs/README.de.md">Deutsch</a> | <a href="./docs/README.tr.md">Türkçe</a> | <a href="./docs/README.ur-pk.md">اردو</a>
</p>

> [!NOTE]
> 🎉 Within one week of launch, OpenHuman became the **number one trending repository on GitHub** for nine days in a row.

> **Early beta.** OpenHuman is under active development, so expect rough edges.

---

## Install

Download the desktop app from [tinyhumans.ai/openhuman](https://tinyhumans.ai/openhuman?utm_source=github&utm_medium=readme) or the [latest release](https://github.com/tinyhumansai/openhuman/releases/latest).

| Platform | Install |
| --- | --- |
| macOS | `brew install --cask openhuman` |
| Windows | The signed `.msi` from the [latest release](https://github.com/tinyhumansai/openhuman/releases/latest) |
| Debian / Ubuntu | `sudo apt-get install ./OpenHuman_*_amd64.deb` after downloading the `.deb` |
| Arch | The [`openhuman-bin`](./packages/arch/openhuman-bin/) AUR recipe |
| Script (macOS, Linux) | `curl -fsSL https://raw.githubusercontent.com/tinyhumansai/openhuman/main/scripts/install.sh \| bash` |

The script does not check signatures, so use a package when you can. Platform notes and fixes are in [INSTALL.md](./INSTALL.md).

---

## Why OpenHuman?

Most agent harnesses run one heavy process per agent and resend a big prompt on every call. OpenHuman does the same work with far less. It is the only feature-rich open-source harness built for large fleets of agents: 500 of them fit on a $10 server.

<table>

<tr>

<td width="50%" valign="top">

<h3>Fast</h3>

<p><a href="https://github.com/tinyhumansai/openhuman-benchmarks/blob/main/results/swe-x86-1/summary.md">Benchmark results</a> · <a href="./gitbooks/developing/performance.md">Cold-start numbers</a></p>

<p>Finishes coding tasks in about 20 seconds, the fastest of seven AI agent tools we tested. Starts up in a tenth of a second.</p>

</td>

<td width="50%" valign="top">

<h3>Cheap</h3>

<p><a href="https://github.com/tinyhumansai/openhuman-benchmarks/blob/main/results/swe-x86-1/summary.md">Cost and token data</a> · <a href="./gitbooks/features/token-compression.md">How compression works</a></p>

<p>Uses 2.6x fewer tokens than the typical agent tool, and had the lowest total bill in our test.</p>

</td>

</tr>

<tr>

<td width="50%" valign="top">

<h3>Efficient at scale</h3>

<p><a href="./gitbooks/developing/performance.md">Fleet measurements</a> · <a href="./docs/library-benchmarking.md">Methodology</a></p>

<p>Uses about 8x less memory and CPU than the typical agent tool. Run more than 500 agents on a $10 server.</p>

</td>

<td width="50%" valign="top">

<h3>Built for developers</h3>

<p><a href="./gitbooks/developing/quickstart.md">Rust quickstart</a> · <a href="./gitbooks/developing/embedding.md">Embedding guide</a> · <a href="./crates/openhuman-embed/examples">Examples</a></p>

<p>Use it as a Rust library: call an agent like any other function, or run a whole fleet from one small server.</p>

</td>

</tr>

</table>

---

## Major innovations

These are the design choices behind those numbers. Each card links to the docs and the code.

<table>

<tr>

<td width="50%" valign="top">

<h3>RLM token compression</h3>

<p><a href="https://arxiv.org/abs/2512.24601">RLM paper</a> · <a href="./gitbooks/features/token-compression.md">How it works</a></p>

<p>Built on <a href="https://arxiv.org/abs/2512.24601">Recursive Language Models</a>. Large tool results get compressed before the AI reads them. For very large ones, the AI gets a handle it can search instead of reading it all. Nothing is thrown away.</p>

</td>

<td width="50%" valign="top">

<h3>Jev: instant, accurate tool search</h3>

<p><a href="./gitbooks/developing/jev.md">Jev</a> · <a href="./docs/plans/jev-tool-search-baseline.md">The measurements</a></p>

<p>Jev is a tiny model that finds the right tool out of 1,215. The right one is in its top picks 86.8% of the time, against 70.5% for keyword search.</p>

</td>

</tr>

<tr>

<td width="50%" valign="top">

<h3>Unified Rust bus</h3>

<p><a href="./gitbooks/developing/loadable-modules.md">How plug-ins work</a></p>

<p>Every feature, like search, documents or voice, plugs into one Rust bus, an idea borrowed from the <a href="https://www.freedesktop.org/wiki/Software/dbus/">Linux system bus</a>. A feature loads only when needed, and if one gets stuck, the rest keep working.</p>

</td>

<td width="50%" valign="top">

<h3>Deeply integrated memory</h3>

<p><a href="./gitbooks/features/memory.md">How memory works</a> · <a href="./gitbooks/developing/engines.md">Memory engines</a></p>

<p>Memory comes built in. Before every turn, OpenHuman picks out only what matters, within a token budget, and hands it to the AI with citations. It works out of the box, and you can swap in a different memory engine with a setting.</p>

</td>

</tr>

<tr>

<td width="50%" valign="top">

<h3>Browser and desktop control</h3>

<p><a href="./gitbooks/features/native-tools/browser-and-computer.md">Browser and computer control</a> · <a href="./gitbooks/developing/jev.md">Jev</a></p>

<p>The agent uses a real browser and your desktop apps. Jev picks each click from the buttons on screen, with no screenshots. It stops before any payment.</p>

</td>

<td width="50%" valign="top">

<h3>A Rust core</h3>

<p><a href="./gitbooks/developing/architecture.md">Architecture</a> · <a href="./gitbooks/developing/performance.md">Performance</a></p>

<p>The whole harness is compiled Rust in one process, with no Node or Python underneath. It starts in a tenth of a second and used 68 MB at peak on our coding tasks. Stripped down, the core is a 51 MB file.</p>

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

We ran seven harnesses on the same SWE-bench tasks, with the same model, API key and container, and metered every call. The setup and results are public in [openhuman-benchmarks](https://github.com/tinyhumansai/openhuman-benchmarks), so anyone can rerun them. The latest run is [`swe-x86-1`](https://github.com/tinyhumansai/openhuman-benchmarks/blob/main/results/swe-x86-1/summary.md), with ten tasks.

OpenHuman finished its tasks in less than half the median time, using 2.6x fewer tokens and an eighth of the memory and CPU. It made 114 model calls where the others made 134 to 212, and its whole run cost $0.05 where the others cost $0.08 to $0.35.

It wins by doing less work per step. The core is compiled Rust in a single process, not Node or Python, so it uses little memory and almost no CPU between model calls. It sends the smallest prompt of the seven, 4.6k tokens with its 20 tools included, so each call is cheaper and comes back faster (1.65 s median, also the fastest). It also needs fewer calls to reach an answer, which is where most of the savings come from. Accuracy is the gap still to close: it solved 7 of 10 tasks, while four others solved all ten.

| Per solved task | OpenHuman (vs. median) | Median of the other six | Best of the other six |
| --- | --- | --- | --- |
| Wall time (p50) | **19.8 s** (2.3x faster) | 46.5 s | 28.4 s (OpenCode) |
| Tokens | **167k** (2.6x fewer) | 435k | 370k (Codex) |
| Cost | **$0.0077** (-36%) | $0.012 | $0.0077 (OpenCode, a tie) |
| Peak memory | **68 MB** (7.7x less) | 523 MB | 123 MB (Codex) |
| CPU time | **1.3 s** (8x less) | 10.4 s | 2.9 s (DeepSeek Harness) |
| System prompt | **4.6k tokens** (-40%) | 7.7k | 6.2k (DeepSeek Harness) |
| Tasks solved | 7 / 10 (3 fewer) | 10 / 10 | 10 / 10 (four harnesses) |

---

## For users

If you use Claude Code, Codex, OpenClaw or Hermes, you already know the ideas: an agent loop with tools, subagents, MCP, skills, BYOK models and persistent memory. OpenHuman has all of them, in a desktop app, a terminal app or a headless server.

| Capability | What OpenHuman ships |
| --- | --- |
| Persistent memory | [RAG over your files, repos, feeds and apps](./gitbooks/features/memory.md), recalled before every turn, answers with citations |
| Tools | [Native tools](./gitbooks/features/native-tools/README.md): shell and coder, web search, scraper, browser and computer use, documents, voice, image and video generation, cron |
| Subagents | An [orchestrator](./gitbooks/features/orchestration.md) that delegates to subagents in agent graphs with checkpoints |
| MCP and skills | [MCP servers and skill bundles](./gitbooks/features/integrations/mcp-and-skills.md), plus [119 managed OAuth integrations](./gitbooks/features/integrations/README.md) and [triggers](./gitbooks/features/integrations/triggers.md) |
| Models | [Local models (Ollama, LM Studio, MLX) and BYOK for 26 providers](./gitbooks/features/model-routing/local-and-byok-models.md), with [automatic model routing](./gitbooks/features/model-routing/README.md) |
| Channels | [14 messaging channels](./gitbooks/features/channels.md) as agent front ends: Telegram, Discord, iMessage, email and more |
| Workflows | [Durable workflow graphs](./gitbooks/features/workflows.md): cron, event or manual triggers, approval steps, resume after a pause |
| Safety | [Approval gate](./gitbooks/features/approval-gate.md), [sandboxed execution](./gitbooks/features/privacy-and-security.md) (OS jail or Docker), [Privacy Mode](./gitbooks/features/privacy-mode.md) for local-only runs, secrets in the [OS keyring](./gitbooks/features/os-keyring-and-secret-storage.md) |
| Observability | Replayable run journals and [per-call cost and usage](./gitbooks/features/billing-and-usage.md) |

Start with [Getting started](./gitbooks/overview/getting-started.md) or the [guides](./gitbooks/guides/README.md).

---

## For developers

OpenHuman is a library-first harness. Add it to your Rust project and call an agent like any other function, with no sidecar or daemon to run. One runtime can hold hundreds of agents, each with its own model, tools, memory and sandbox. 500 of them fit on a $10 VPS, so a product with one agent per customer can start without a cluster.

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

Next: the [Rust quickstart](./gitbooks/developing/quickstart.md), the [embedding guide](./gitbooks/developing/embedding.md) and the [developer docs](./gitbooks/developing/README.md).

---

## How it compares

Products change quickly, so check each project before you decide.

|                         | Claude Cowork     | OpenClaw          | Hermes Agent      | OpenHuman                                                          |
| ----------------------- | ----------------- | ----------------- | ----------------- | ------------------------------------------------------------------ |
| **Open source**         | 🚫 Proprietary    | ✅ MIT            | ✅ MIT            | ✅ GPL-3.0                                                         |
| **Simple to start**     | ✅ Desktop + CLI  | ⚠️ Terminal-first | ⚠️ Terminal-first | ✅ Clean UI, minutes                                               |
| **Cost**                | ⚠️ Sub + add-ons  | ⚠️ BYO models     | ⚠️ BYO models     | ✅ 2.6x fewer tokens                                               |
| **Agent fleets**        | 🚫 One session    | ⚠️ Process per agent | ⚠️ Process per agent | 🚀 500 agents on a $10 VPS                                    |
| **Embeddable library**  | ⚠️ SDK over a CLI | 🚫 None           | ⚠️ Python package | 🚀 Typed Rust API                                                  |
| **Memory**              | ✅ Chat-scoped    | ⚠️ Plugin-reliant | ✅ Self-learning  | 🚀 Recalled every turn, with citations                            |
| **Integrations**        | ⚠️ Few connectors | ⚠️ BYO            | ⚠️ BYO            | 🚀 119 OAuth apps, MCP, skills                                     |
| **Source sync**         | 🚫 None           | 🚫 None           | 🚫 None           | ✅ Folders, repos, feeds, apps                                      |
| **Orchestration**       | ⚠️ Sub-tasks      | ⚠️ Single loop    | ⚠️ Single loop    | 🚀 Agent graphs with checkpoints                                   |
| **Workflows**           | 🚫 None           | ⚠️ Scripts        | ⚠️ Scripts        | 🚀 Visual, agent-drafted                                           |
| **Messaging channels**  | 🚫 None           | ✅ Many           | ✅ Several        | ✅ 14, including email                                             |
| **Local-only mode**     | 🚫 Cloud-only     | ⚠️ BYO local      | ⚠️ BYO local      | ✅ One-switch Privacy Mode                                         |
| **Observability**       | 🚫 Opaque         | ⚠️ Logs           | ⚠️ Logs           | ✅ Run journals, per-call cost                                     |
| **Public benchmarks**   | 🚫 None           | 🚫 None           | 🚫 None           | ✅ [Public, reproducible](https://github.com/tinyhumansai/openhuman-benchmarks) |
| **Model routing**       | 🚫 Single vendor  | ⚠️ Manual         | ⚠️ Manual         | ✅ Built-in                                                        |
| **Native tools**        | ✅ Code-focused   | ✅ Code-focused   | ✅ Code-focused   | ✅ Code, search, browser, voice, media                             |

---

## Contributing

Read [`CONTRIBUTING.md`](./CONTRIBUTING.md), or let an AI coding agent guide you with [this prompt](./docs/CONTRIBUTING-BEGINNERS.md#optional--let-an-ai-coding-agent-guide-you).

1. Install Git, Node.js 24+, pnpm 10.10.0, Rust 1.96.1 (with `rustfmt` and `clippy`), CMake, Ninja, ripgrep, and your platform's desktop build prerequisites.
2. Fork and clone the repo. Run `git submodule update --init --recursive`, then `pnpm install`.
3. Run `pnpm dev` for UI work or `pnpm dev:app` for the desktop app. Before you open a PR, run `pnpm typecheck`, `pnpm format:check` and `cargo check --manifest-path Cargo.toml`.

More in [Getting set up](./gitbooks/developing/getting-set-up.md), [`AGENTS.md`](./AGENTS.md) and the [crates overview](./crates/README.md). Many parts of OpenHuman live in their own repos under [`vendor/`](./vendor), and they welcome contributions too.

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
