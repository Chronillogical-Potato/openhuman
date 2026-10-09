<h1 align="center">OpenHuman</h1>

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

Most agent harnesses are a Node or Python process per agent, a large system prompt, and a bill that grows with every turn. OpenHuman is a Rust core built to do the same work with less of everything: fewer tokens sent to the model, less memory per agent, less CPU per task. The same core runs a desktop app for people and a library for developers who want to put hundreds of agents behind one product.

<table>
<tr>
<td width="33%" valign="top">

### Fast

Fastest wall time in the latest public SWE-bench run: **20 s** per solved task at the median, against 28 to 62 s for the other six harnesses. The first token comes back in 1.2 s, also the fastest. An agent turn starts cold in 102 ms.

</td>
<td width="33%" valign="top">

### Cheap

**2.6x fewer tokens** per solved task than the median harness, and the lowest total spend of the run ($0.05 for all ten tasks, against $0.08 to $0.35). The smallest static prompt (4.6k tokens), and [token compression](./gitbooks/features/token-compression.md) shrinks tool output before it reaches the model.

</td>
<td width="33%" valign="top">

### Efficient at scale

**68 MB** peak memory and **1.3 CPU-seconds** per solved task, about 8x less than the median harness. In one process, each extra agent costs about 1.8 MiB: [500 agents measured](./gitbooks/developing/performance.md) in 1.4 GiB, roughly 25 times denser than 500 separate processes.

</td>
</tr>
</table>

That last column is the point. OpenHuman is the only feature-rich open-source harness built to run large fleets of agents. Claude Code, Codex, OpenCode, OpenClaw and Hermes run one agent per process, so a hundred agents means a hundred runtimes, a hundred heaps and a hundred copies of the system prompt to keep warm. OpenHuman runs the hundred as values inside one Rust process, each with its own model, tools, memory and sandbox, and still ships memory, integrations, channels, workflows and a desktop app on top.

It is also programmable all the way down. Every engine (LLM, embeddings, memory, search) is chosen by config, every capability is a Cargo feature or a lazily loaded native module, and the whole core is a typed Rust API: [`openhuman-embed`](./crates/openhuman-embed/README.md).

---

## Benchmarks you can check

<p align="center">
 <picture>
  <source media="(prefers-color-scheme: dark)" srcset="./gitbooks/.gitbook/assets/benchmarks/swe-x86-1-dark.png" />
  <source media="(prefers-color-scheme: light)" srcset="./gitbooks/.gitbook/assets/benchmarks/swe-x86-1.png" />
  <img alt="SWE-bench Verified run swe-x86-1: OpenHuman against Claude Code, Codex, OpenCode, OpenClaw, Hermes and DeepSeek Harness on ten panels (tasks solved, cost, tokens, prompt size, cache hit, time to first token, wall time, cold start, CPU, RAM)" src="./gitbooks/.gitbook/assets/benchmarks/swe-x86-1.png" />
 </picture>
</p>

Every number above comes from [tinyhumansai/openhuman-benchmarks](https://github.com/tinyhumansai/openhuman-benchmarks), a separate public repository. It runs OpenHuman, Claude Code, Codex, OpenCode, OpenClaw, Hermes and DeepSeek Harness on the same SWE-bench Verified tasks, and it holds them to the same conditions:

- The same model (`deepseek/deepseek-v4.1-flash`, reasoning `high`), pinned to one provider with fallbacks off, so the prompt cache is never split.
- The same OpenRouter key, held only by a metering proxy that sits between every harness and the model. Tokens, latency, cache hits and cost are read off the wire, not from each harness's own accounting.
- The same container: 4 vCPU, 8 GB, no swap, one harness at a time on a quiet host.
- The same grader: the official `swebench.harness.run_evaluation`.

Results, per-task patches, harness logs and the per-call meter records are committed. The [results site](https://tinyhumansai.github.io/openhuman-benchmarks/) rebuilds on every push, and [`RUNBOOK.md`](https://github.com/tinyhumansai/openhuman-benchmarks/blob/main/RUNBOOK.md) reproduces a run on your own machine with your own key. OpenHuman is vendored there as a submodule, so moving the pin benchmarks any build.

| `swe-x86-1`, per solved task | OpenHuman | Median of the other six | Best of the other six |
| --- | --- | --- | --- |
| Wall time (p50) | **19.8 s** | 46.5 s | 28.4 s (OpenCode) |
| Tokens | **167k** | 435k | 370k (Codex) |
| Cost | $0.0077 | $0.012 | $0.0077 (OpenCode, a tie) |
| Peak process RAM | **68 MB** | 523 MB | 123 MB (Codex) |
| CPU time | **1.3 s** | 10.4 s | 2.9 s (DeepSeek Harness) |
| Static prompt (system + tools) | **4.6k tokens** | 7.7k | 6.2k (DeepSeek Harness) |
| Tasks resolved | 7 / 10 | 10 / 10 | 10 / 10 (four harnesses) |

The density gap is the part of these results that compounds. At 68 MB and 1.3 CPU-seconds per solved task, against a median of 523 MB and 10.4 s, the same box holds several times as many OpenHuman agents before it runs out of memory or cores. Inside one process the gap widens further, because each extra agent shares the runtime and costs about 1.8 MiB.

Read the last row too. OpenHuman spends its context well, but it resolved fewer tasks than four of the six, and closing that gap is the current work. Ten tasks is a smoke test, so a one-task difference is noise. In this run the task containers also had internet access, which the benchmark repository flags as a contamination risk; later runs sit on a network that reaches only the meter proxy. The raw rows are in [`results/swe-x86-1/summary.md`](https://github.com/tinyhumansai/openhuman-benchmarks/blob/main/results/swe-x86-1/summary.md), and the in-process numbers (agent density, cold start, binary size) are in [performance](./gitbooks/developing/performance.md) and [`docs/library-benchmarking.md`](./docs/library-benchmarking.md).

---

## For users

<p align="center">
 <img src="./demo.gif" alt="A walkthrough of the OpenHuman desktop app" />
</p>

Download the desktop app for Windows, macOS or Linux from [tinyhumans.ai/openhuman](https://tinyhumans.ai/openhuman?utm_source=github&utm_medium=readme) or [GitHub Releases](https://github.com/tinyhumansai/openhuman/releases/latest). Homebrew, `.deb`, AUR and install scripts are in [INSTALL.md](./INSTALL.md), and the first ten minutes are in [Getting started](./gitbooks/overview/getting-started.md).

What you can do with it:

- [Chat](./gitbooks/features/chat.md) with an assistant that sees your files and apps, and hand bigger jobs to [the orchestrator](./gitbooks/features/orchestration.md), which splits them across sub-agents.
- Give it a [memory](./gitbooks/features/memory.md) of your documents, folders, repos, feeds and connected apps. It recalls what matters before each turn and cites its sources.
- Connect [119 apps through one-click OAuth](./gitbooks/features/integrations/README.md), add [MCP servers and skills](./gitbooks/features/integrations/mcp-and-skills.md), and fire agents from [triggers](./gitbooks/features/integrations/triggers.md).
- Describe an automation and let the agent draft it as a [workflow](./gitbooks/features/workflows.md) you review on a canvas, built on the open [tinyflows](https://github.com/tinyhumansai/tinyflows) engine.
- Talk to it where you already are: 14 [messaging channels](./gitbooks/features/channels.md) in the shipped build, including Telegram, Discord, iMessage and native email.
- Use the [native tools](./gitbooks/features/native-tools/README.md): [web search](./gitbooks/features/native-tools/web-search.md), a [scraper](./gitbooks/features/native-tools/web-scraper.md), a [coder](./gitbooks/features/native-tools/coder.md), [documents](./gitbooks/features/native-tools/documents.md), [browser and computer control](./gitbooks/features/native-tools/browser-and-computer.md), [voice](./gitbooks/features/native-tools/voice.md), [image and video generation](./gitbooks/features/native-tools/media-generation.md) and [scheduling](./gitbooks/features/native-tools/cron.md).
- Run on any model: the managed route, [local models or your own key](./gitbooks/features/model-routing/local-and-byok-models.md) for 26 providers, with [automatic routing](./gitbooks/features/model-routing/README.md) between them.
- Keep data on your machine with one-switch [Privacy Mode](./gitbooks/features/privacy-mode.md), secrets in the [OS keyring](./gitbooks/features/os-keyring-and-secret-storage.md), and risky actions behind the [approval gate](./gitbooks/features/approval-gate.md).
- Track [goals and todos](./gitbooks/features/goals-and-todos.md), let it [learn your preferences](./gitbooks/features/personalization.md), and watch [cost and usage](./gitbooks/features/billing-and-usage.md) per call.
- Make it yours with [themes](./gitbooks/features/theming.md) and [the mascot](./gitbooks/features/mascot/README.md), or take it further with the [wallet](./gitbooks/features/wallet.md), [hosting](./gitbooks/features/hosting.md) and the [iOS companion](./gitbooks/features/ios-companion.md).

Not sure where to start? The [guides](./gitbooks/guides/README.md) walk through real setups: a [personal assistant](./gitbooks/guides/personal-assistant.md), a [fully local model](./gitbooks/guides/local-model.md), [private data](./gitbooks/guides/privacy-sensitive-data.md) and more.

---

## For developers

OpenHuman is a library first. The desktop app, the browser UI, the terminal client and the JSON-RPC server are all hosts around the same core, and your product can be one more.

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

That is one agent. A [`Runtime`](./crates/openhuman-embed/README.md) holds any number of them in one process, each with its own model, access tier, working directory, MCP servers, skills and sandbox, and each costing about 1.8 MiB of memory once the first is up. One [TinyHumans API key](./gitbooks/developing/tinyhumans-api-key.md) turns on managed inference, search, embeddings, voice and integrations for all of them, or you bring your own providers.

```text
   your product        desktop app       terminal (TUI)     JSON-RPC / CLI
        |                   |                  |                   |
        v                   v                  v                   v
  openhuman-embed     openhuman-app      openhuman-tui       openhuman-rpc
        |                   |                  |                   |
        +---------+---------+------------------+-------------------+
                  |
                  v
           openhuman-core   agents, memory, tools, security, channels, flows
                  |
     +------------+-------------+--------------------+
     v            v             v                    v
 tinyagents   tinymemory   native modules       backend transport
 (agent loop) (memory)     (search, docs,       (openhuman-tinyhumans,
                            browser, voice...)   optional)
```

Where to go next:

- [Rust quickstart](./gitbooks/developing/quickstart.md): from an empty crate to many agents on one runtime, step by step.
- [Embedding reference](./gitbooks/developing/embedding.md): `Runtime`, `AgentSpec`, access tiers, providers, MCP, skills, sessions and narrow builds.
- [Architecture](./gitbooks/developing/architecture/README.md) and the [agent harness](./gitbooks/developing/architecture/agent-harness.md): how a turn runs, from prompt to tool call to transcript.
- [Pluggable engines](./gitbooks/developing/engines.md) and [loadable modules](./gitbooks/developing/loadable-modules.md): swap the LLM, memory or search engine, and add capabilities without recompiling.
- [Performance](./gitbooks/developing/performance.md): the agent-density sweep, cold start and binary sizes, with methodology.
- [Building the Rust core](./gitbooks/developing/building-rust-core.md) and the [crates overview](./crates/README.md): what each crate in the workspace is for.

---

## How it compares

Measured, from the public [`swe-x86-1`](https://github.com/tinyhumansai/openhuman-benchmarks/blob/main/results/swe-x86-1/summary.md) run (same model, same key, same container; lower is better except tasks resolved):

| | OpenHuman | Claude Code | Codex | OpenCode | OpenClaw | Hermes |
| --- | --- | --- | --- | --- | --- | --- |
| Tasks resolved | 7/10 | 10/10 | 7/10 | 10/10 | 9/10 | 10/10 |
| Wall time per solved task (p50) | **19.8 s** | 37.8 s | 36.1 s | 28.4 s | 62 s | 58.5 s |
| Tokens per solved task | **167k** | 428k | 370k | 390k | 442k | 482k |
| Total cost, 10 tasks | **$0.05** | $0.35 | $0.12 | $0.08 | $0.10 | $0.08 |
| Peak process RAM | **68 MB** | 231 MB | 123 MB | 1.06 GB | 1.57 GB | 816 MB |
| CPU time per solved task | **1.3 s** | 4.7 s | 3.3 s | 17.2 s | 26.7 s | 16.1 s |
| Static prompt (system + tools) | **4.6k** | 12.0k | 7.7k | 6.7k | 7.6k | 13.2k |
| Cold start to first model call | 0.8 s | 0.3 s | **0.2 s** | 1.2 s | 5.5 s | 10.0 s |

By design, as of October 2026 (these projects move fast, so check each one before relying on a row):

| | OpenHuman | Claude Code / Codex | OpenCode | OpenClaw / Hermes |
| --- | --- | --- | --- | --- |
| License | GPL-3.0 | Proprietary / Apache-2.0 | MIT | MIT |
| Core language | Rust | TypeScript / Rust | TypeScript | TypeScript / Python |
| Embed in your own program | Typed Rust API, many agents in one process | SDKs that drive a CLI process | Server API and SDK | Run as a separate process |
| Any model, local or BYOK | 26 providers plus local servers | Vendor models first | Yes | Yes |
| Persistent memory with citations | Pluggable engine, recalled every turn | Project notes files | Project notes files | Plugin or self-learning |
| Managed OAuth integrations | 119 apps, plus MCP | MCP | MCP | Bring your own, plus MCP |
| Messaging channels | 14 in the shipped build | No | No | Yes |
| Visual workflows | Agent-drafted, reviewed on a canvas | No | No | Scripts |

---

## Contributing

New here? [`CONTRIBUTING.md`](./CONTRIBUTING.md) covers the fork and PR workflow and the local checks, and [`CONTRIBUTING-BEGINNERS.md`](./docs/CONTRIBUTING-BEGINNERS.md#optional--let-an-ai-coding-agent-guide-you) has a copy-paste prompt that lets an AI coding agent walk you through your first change.

The short path:

1. Install Git, Node.js 24+, pnpm 10.10.0, Rust 1.96.1 (with `rustfmt` and `clippy`), CMake, Ninja, ripgrep, and your platform's desktop build prerequisites.
2. Fork and clone, then run `git submodule update --init --recursive` before `pnpm install` so the vendored crates under `vendor/` resolve.
3. Run `pnpm dev` for UI work or `pnpm dev:app` for the desktop shell, and check your change with `pnpm typecheck`, `pnpm format:check` and `cargo check --manifest-path Cargo.toml` before opening a PR.

[Getting set up](./gitbooks/developing/getting-set-up.md) has the full environment guide, [`AGENTS.md`](./AGENTS.md) has the conventions the codebase follows, and the [crates overview](./crates/README.md) maps the workspace. Much of OpenHuman lives in its own open repositories ([tinyagents](https://github.com/tinyhumansai/tinyagents), [tinyflows](https://github.com/tinyhumansai/tinyflows), [tinymemory](https://github.com/tinyhumansai/tinymemory) and more under [`vendor/`](./vendor)), and contributions there are just as welcome.

Contributors get free merch and special access on [Discord](https://guild.tinyhumans.ai/).

## Where this is heading

The open work is tracked in public issues: leaving beta ([#6047](https://github.com/tinyhumansai/openhuman/issues/6047)), an agent-to-agent protocol ([#3463](https://github.com/tinyhumansai/openhuman/issues/3463)), pluggable memory adapters ([#5390](https://github.com/tinyhumansai/openhuman/issues/5390)), external agent runtimes over ACP ([#4731](https://github.com/tinyhumansai/openhuman/issues/4731)) and a managed local model runtime ([#6130](https://github.com/tinyhumansai/openhuman/issues/6130)). Thousands of agents on one box is the direction; 500 is what has been measured. See the [roadmap](./gitbooks/overview/roadmap.md).

## License

OpenHuman is licensed under [GPL-3.0](./LICENSE).

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
