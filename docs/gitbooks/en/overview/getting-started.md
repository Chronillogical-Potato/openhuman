---
description: >-
  Install OpenHuman, walk through the in-app onboarding (sign in, choose how AI
  runs, connect an app), and run your first request against your own memory.
icon: play
---

# Getting Started

This page walks you through installing OpenHuman, going through the in-app onboarding, and running your first request.

OpenHuman is open source under the GNU GPL3 license. The codebase is at [github.com/tinyhumansai/openhuman](https://github.com/tinyhumansai/openhuman).

{% hint style="info" %}
**Want a specific outcome?** If you're here to accomplish something concrete (set up a private assistant, run a local model, recover a broken install, or move to a new machine), the [Guides](../guides/guides/) section has task-by-task walkthroughs.
{% endhint %}

***

## System requirements

OpenHuman runs on **macOS, Windows and Linux** desktops. 4 GB+ RAM is recommended; 16 GB+ if you intend to ingest very large mailboxes or repos, or run a [local model](../features/model-routing/local-ai.md) on the same machine (you install the runtime, such as Ollama, and pull the models yourself).

### Permissions

The first time you launch OpenHuman, the OS will prompt for the permissions the app needs (Accessibility on macOS, Input Monitoring for the voice hotkey). You can review and adjust these any time under **Settings**.

***

## 1. Download and install

Get the OpenHuman desktop app from [tinyhumans.ai/openhuman](https://tinyhumans.ai/openhuman) or via your platform's package manager. Open the app once it's installed.

## 2. Sign in

The first screen is **"Sign in! Let's Cook"**. Multiple sign-in options are available, including social login. There's also an **Advanced** panel for pointing the app at a custom core RPC URL if you're running your own backend; most users can ignore it.

{% hint style="info" %}
**No permanent lock-in.** Signing in does not grant OpenHuman ongoing access to anything. All third-party access requires explicit OAuth approval per integration in the steps below.
{% endhint %}

{% hint style="warning" %}
**Know what is local and what is managed.** Your workspace config, and local runtime state live on your machine. The default setup still uses OpenHuman-hosted services for sign-in, model routing, managed integration OAuth/tool calls, and web search proxying. Use the custom setup paths if you want to bring your own model, search, or Composio credentials. Some hosted features and real-time integration triggers still require the managed backend.
{% endhint %}

## 3. Connect something, and set up memory

Onboarding offers a runtime choice (the managed TinyHumans route, or a custom setup where you bring your own model, search, embeddings and connector keys) and then drops you into the app. Two things make the first request worth making:

* **An integration.** Open [Connections](../features/connections.md) → Apps and connect one. Gmail is the usual first choice; each connection is a one-click OAuth approval you can revoke.
* **A memory engine.** Connections → Memory. Signed in, the hosted engine is already there; otherwise point it at your own CortexDB. With neither, [memory](../features/memory.md) is off and the agent starts from zero every chat.

Add the integration as a memory source on the Brain tab to sync it on a schedule.

## 4. Run your first request

Once a memory source has synced, try prompts like:

**Briefings**

* "What do I need to know from the last 12 hours?"
* "What's waiting on me?"

**Cross-source queries**

* "Summarize what I missed today."
* "What are the key decisions from this week?"
* "Extract action items from my recent conversations."
* "What did Sarah say about the project across email and chat?"

OpenHuman picks the right model for each task automatically. See [Automatic Model Routing](../features/model-routing/).

***

## 5. Keep going

Now that the agent has memory and a model, the rest of the product is about giving it more surfaces:

* [**Chat**](../features/chat.md): what a turn can actually do, and the four ways it stops to ask you something.
* [**Connections**](../features/connections.md): everything the agent plugs into, on one page.
* [**Memory**](../features/memory.md): connect more sources; they sync on a schedule into your memory engine.
* [**Workflows**](../features/workflows.md): describe an automation and review the graph the agent proposes.
* [**Native Voice**](../features/native-tools/voice.md): dictation and spoken replies, or a live voice agent.
* [**Roadmap**](roadmap.md): what is coming, and what was removed.

## Join the community

OpenHuman is in early beta. Feedback and contributions make a real difference at this stage.

* **GitHub:** [github.com/tinyhumansai/openhuman](https://github.com/tinyhumansai/openhuman)
* **Discord:** [guild.tinyhumans.ai](https://guild.tinyhumans.ai)
