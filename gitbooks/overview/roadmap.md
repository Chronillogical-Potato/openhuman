---
description: >-
  What is being worked on next, and the known gaps. Tracked in public issues
  rather than promised here.
icon: map
---

# Roadmap

This page is a reading of the open issue tracker, not a commitment. Dates are deliberately absent. Everything below links to the issue that owns it, so if this page goes stale the issue is still the truth.

Checked 2026-10-07.

## Taking off the early-beta badge

[#6047](https://github.com/tinyhumansai/openhuman/issues/6047) is the checklist, with an explicit definition of done: the badge comes off when the boxes are closed, the memory smoke test passes on macOS, Windows and Linux, and the release workflow can publish without hand-holding. Read the boxes against the text beside them: a ticked box is not always a passing test.

| Area | State |
| --- | --- |
| Memory end to end (store, restart, recall, and the agent actually uses it) | Two of three items ticked ([#6041](https://github.com/tinyhumansai/openhuman/issues/6041), [#6040](https://github.com/tinyhumansai/openhuman/issues/6040)); the third is the end-to-end smoke test, which the checklist itself records as not currently passing |
| Reply persistence: a completed reply can never vanish | Done |
| Clean-install sign-in on all three platforms | Open, [#6020](https://github.com/tinyhumansai/openhuman/issues/6020) |
| Windows: native modules refused when `%TEMP%` grants Modify to a non-owner group, leaving memory and connectors unavailable | Open, [#6008](https://github.com/tinyhumansai/openhuman/issues/6008) |

The release-pipeline and Windows path-length blockers the checklist also lists have since been closed, even though the boxes are still unticked there.

## Programmes with several issues behind them

- **Agent to agent.** [#3463](https://github.com/tinyhumansai/openhuman/issues/3463) is the umbrella over five phases: an agent-card endpoint and discovery, accepting tasks, delegating to an external agent as a tool, streaming and push notifications, and multi-instance coordination.
- **Pluggable memory adapters.** [#5390](https://github.com/tinyhumansai/openhuman/issues/5390) adds the adapters directory and the dependency rule; supermemory, mem0, agentmemory and cognee follow behind it. Today [memory](../features/memory.md) has two engines and no adapter directory exists yet.
- **External agent runtimes.** [#4731](https://github.com/tinyhumansai/openhuman/issues/4731) is an abstraction layer for driving Cursor, Windsurf and Codex, with a shared ACP transport for several more and per-session model and effort selection. Session import from one of them is the live edge of this work.
- **Local and self-hosted.** [#6130](https://github.com/tinyhumansai/openhuman/issues/6130) would bring back a managed local runtime with model downloads and a compute-backend selector, deliberately re-adding something that was removed: local inference today means an endpoint you run yourself. [#4844](https://github.com/tinyhumansai/openhuman/issues/4844) is a no-UI Linux build for a server or a Raspberry Pi.
- **Observability.** [#4496](https://github.com/tinyhumansai/openhuman/issues/4496) adds scores to traces, so user feedback and automated quality signals land next to the run they describe.

## Shipped but inert

Two surfaces exist in the build and do nothing, which is worth knowing before you go looking for them:

- **Follow-up suggestion chips** are built and unreachable: nothing produces them, so the row is always empty ([#6465](https://github.com/tinyhumansai/openhuman/issues/6465)).
- **SearXNG** is configurable and reachable over the MCP server, but `searxng_search` is not registered as an agent tool, so the agent in chat cannot use it ([#6127](https://github.com/tinyhumansai/openhuman/issues/6127)).

The app also ships its own maturity ledger: **Settings → About** lists every capability with a status of stable, beta or coming soon, which is a better answer than this page for "is X finished".

## Recently removed

Things the docs used to describe that are gone, so you are not hunting for them:

- The **live Google Meet agent**. It joined a call through the old embedded Chromium webview and spoke back as a camera stream. The frontend, the domain and the join path were all removed. See [Voice](../features/native-tools/voice.md).
- The **embedded Chromium (CEF) runtime**. The shell runs on Tauri's native webview; browser control is a real Chrome driven over the DevTools protocol. See [Browser & Computer Control](../features/native-tools/browser-and-computer.md) and the historical [CEF notes](../developing/cef.md).
- **In-app model downloads** and runtime lifecycle management for local models. Point OpenHuman at a server you run.
- The **skills sandbox**. Skills are a catalogue you browse and install plus a run surface, not code executing inside a JavaScript sandbox in the app.
- The **memory tree, graph view, people list and `MEMORY.md`**, replaced by the per-turn memory pack. [Memory](../features/memory.md) has the full list.

## See also

- [Release Policy](../developing/release-policy.md): how a release is cut and what the version gate does.
- [Platform & Availability](../features/platform.md): what runs where today.
