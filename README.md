<h1 align="center">OpenHuman</h1>

<p align="center">
 <img src="./demo.gif" alt="A walkthrough of the OpenHuman desktop app" />
</p>

<p align="center">
 <strong>The fast, cheap, open-source agent harness. Run 500 agents on a $10 VPS.</strong><br/>
 A desktop app for people. A Rust library for developers.
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

## Install

Download the desktop app from [tinyhumans.ai/openhuman](https://tinyhumans.ai/openhuman?utm_source=github&utm_medium=readme) or the [latest release](https://github.com/tinyhumansai/openhuman/releases/latest).

| Platform | Install |
| --- | --- |
| macOS | `brew install --cask openhuman` |
| Windows | The signed `.msi` from the [latest release](https://github.com/tinyhumansai/openhuman/releases/latest) |
| Debian / Ubuntu | `sudo apt-get install ./OpenHuman_*_amd64.deb` after downloading the `.deb` |
| Arch | The [`openhuman-bin`](./packages/arch/openhuman-bin/) AUR recipe |
| Script (macOS, Linux) | `curl -fsSL https://raw.githubusercontent.com/tinyhumansai/openhuman/main/scripts/install.sh \| bash` |

The script path has no signature check, so prefer a package where you can. Platform notes and troubleshooting are in [INSTALL.md](./INSTALL.md).

---

## Why OpenHuman

Most harnesses run one heavy process per agent and resend a big prompt on every call. OpenHuman is a Rust core that does the same work with less of everything, and it is the only feature-rich open-source harness built to run large fleets of agents. Where other harnesses need a beefy machine per handful of agents, OpenHuman runs 500 on a $10 server.

<table>
<tr>
<td width="50%" valign="top">

### Fast

**20 s** per solved SWE-bench task, the fastest of seven harnesses (others: 28 to 62 s). Cold start in 102 ms.

</td>
<td width="50%" valign="top">

### Cheap

**2.6x fewer tokens** per solved task and the lowest total spend. Small prompts, plus [token compression](./gitbooks/features/token-compression.md) on tool output.

</td>
</tr>
<tr>
<td width="50%" valign="top">

### Efficient at scale

**8x less** memory and CPU per task. Each extra agent costs about 1.8 MiB: [500 agents](./gitbooks/developing/performance.md) fit in 1.4 GiB, which is a $10, 2 GB VPS.

</td>
<td width="50%" valign="top">

### Built for developers

A library first. Call an agent like a function from your Rust code, or run a fleet of full-featured agents from one small server.

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

Seven harnesses, the same SWE-bench tasks, the same model, key and container, every call metered on the wire. It is all public and reproducible in [openhuman-benchmarks](https://github.com/tinyhumansai/openhuman-benchmarks). Latest run: [`swe-x86-1`](https://github.com/tinyhumansai/openhuman-benchmarks/blob/main/results/swe-x86-1/summary.md), ten tasks.

| Per solved task | OpenHuman | Others (median) |
| --- | --- | --- |
| Wall time | **19.8 s** | 46.5 s |
| Tokens | **167k** | 435k |
| Memory | **68 MB** | 523 MB |
| CPU | **1.3 s** | 10.4 s |
| Tasks solved | 7 / 10 | 10 / 10 |

---

## For users

An assistant that knows your work, acts in your apps and keeps your data yours. [Install it](#install) and you are chatting in minutes.

| You want to | You get |
| --- | --- |
| Stop repeating yourself | [Memory](./gitbooks/features/memory.md) of your files, repos and apps, with citations |
| Get work done | [Native tools](./gitbooks/features/native-tools/README.md) and an [orchestrator](./gitbooks/features/orchestration.md) for big jobs |
| Use your apps | [119 integrations](./gitbooks/features/integrations/README.md), [MCP and skills](./gitbooks/features/integrations/mcp-and-skills.md) |
| Chat from anywhere | [14 channels](./gitbooks/features/channels.md): Telegram, Discord, iMessage, email |
| Automate | [Workflows](./gitbooks/features/workflows.md) the agent drafts for you |
| Pick your model | [Local or your own key](./gitbooks/features/model-routing/local-and-byok-models.md), with [auto routing](./gitbooks/features/model-routing/README.md) |
| Stay private | [Privacy Mode](./gitbooks/features/privacy-mode.md) and an [approval gate](./gitbooks/features/approval-gate.md) |

Start with [Getting started](./gitbooks/overview/getting-started.md) or the [guides](./gitbooks/guides/README.md).

---

## For developers

OpenHuman is a library-first harness. Add it to your Rust codebase and call an agent like any other function. No sidecar, no daemon. When you need more, one runtime holds hundreds of agents, each with its own model, tools, memory and sandbox. A fleet of 500 runs on a $10 VPS, so a product with an agent per customer does not need a cluster to start.

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

Products change, so verify against each project.

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
2. Fork and clone, then run `git submodule update --init --recursive` before `pnpm install` so the vendored crates under `vendor/` resolve.
3. Run `pnpm dev` for UI work or `pnpm dev:app` for the desktop shell, and check your change with `pnpm typecheck`, `pnpm format:check` and `cargo check --manifest-path Cargo.toml` before opening a PR.

More: [Getting set up](./gitbooks/developing/getting-set-up.md), [`AGENTS.md`](./AGENTS.md) and the [crates overview](./crates/README.md). Much of OpenHuman lives in its own repos under [`vendor/`](./vendor), and those welcome contributions too.

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
