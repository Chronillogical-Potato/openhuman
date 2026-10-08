---
description: >-
  A plain-language map of what OpenHuman keeps on your computer versus what it
  sends out, and the settings that let you keep sensitive work local.
icon: lock
---

# Keep sensitive data private

**Goal:** understand, in everyday language rather than architecture, what stays on your machine and what leaves it, so you can decide what OpenHuman should touch.

If you want the engineering detail, read [Privacy & Security](../features/privacy-and-security.md). This guide is the version you can act on in five minutes.

---

## The one-sentence version

**Your settings, files and secrets stay on your computer; your memory is stored in CortexDB (hosted by TinyHumans for your account, or your own). The OpenHuman backend handles what has to be brokered: signing you in, hosted memory, routing model requests, and talking to the services you connect.**

Everything below is an expansion of that sentence.

---

## What stays on your machine

These never leave your computer as raw data:

| Thing                         | Plain meaning                                                                                |
| ----------------------------- | -------------------------------------------------------------------------------------------- |
| **Audio you speak**           | Captured to transcribe, then discarded.                                                      |
| **Local model state**         | If you use [local AI](local-model.md), your own runtime and its models stay on-device.       |
| **Your persona and settings** | The files that define how your assistant behaves and what it's allowed to do.                |

## Your memory is stored off your machine

Documents, conversations, learnings and the beliefs built from them are stored in **CortexDB**, not on your computer:

- **TinyHumans engine (default when signed in):** stored in the hosted CortexDB that TinyHumans runs, through the TinyHumans backend. Each account gets its own isolated tenant, so other users cannot see your memory. TinyHumans operates that service.
- **CortexDB engine:** stored in your own CortexDB account or self-hosted CortexDB, reached directly with your key.

Secrets and personal identifiers are scrubbed before an item is sent. You can forget single items or whole sources from the Memory page; see [Deleting memory](../features/privacy-and-security.md#deleting-memory) for erasing everything.

## What the backend handles (and why)

These leave your machine because they can't work otherwise, but note _what_ is sent:

| Thing                  | What's actually sent                                                                                                                                                               |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Hosted memory**      | With the TinyHumans engine, every item stored and every recall goes through the backend to hosted CortexDB.                                                                       |
| **Model requests**     | Only what the assistant needs for that turn: your prompt plus the memory recalled for it. Not your whole memory.                                                                    |
| **Web search**         | Your search query goes to the backend proxy (so you don't need your own search key).                                                                                               |
| **Connected services** | When you connect Gmail, Slack, etc., the backend brokers each request. Your login tokens for those services are held by the backend, **not written in plain text on your laptop**. |
| **Text-to-speech**     | The words to be spoken are streamed to generate audio, then discarded. They are not retained.                                                                                      |

{% hint style="info" %}
**Memory is not local, so scrubbing and scoping matter.** Your memory engine stores items in CortexDB and answers recall there. What protects you is that items are scrubbed before they are sent, tool-call arguments are never stored, hosted memory is isolated per account, and a model turn sees only what was recalled for it.
{% endhint %}

## Two promises worth knowing

- **No training on your data.** Your conversations, memory, and personal information are never used to train models.
- **Secrets are stored by your operating system.** Local secrets are kept in your platform's secure store (macOS Keychain, Windows Credential Manager, or the Linux Secret Service), not lying around in app files. See [OS Keyring & Secret Storage](../features/os-keyring-and-secret-storage.md).

---

## Turning the dial toward "more local"

You have real controls. From most to least private:

1. **Route inference on-device.** Run a local runtime such as Ollama yourself, pull the models, and [add it as a provider](local-model.md) so embeddings, summarization, and optionally chat/reasoning happen on your machine. OpenHuman doesn't install the runtime or download models for you. _(Speech and web search still use the backend proxy even then.)_
2. **Tighten what the assistant can do.** Set `[autonomy] enabled = true` and `level = "readonly"` in `config.toml`: it can then observe and answer but never act or reach the network on its own. The policy is **off by default**, so this is a switch you have to throw, not one to leave alone. See the [Approval Gate](../features/approval-gate.md).
3. **Keep it in one folder.** The filesystem boundary has two preconditions: the policy enabled, and `workspace_only` on. With both, the agent is confined to its working folder and cannot read the rest of your disk. With either one off, that boundary is not enforced. A **trusted root** is the deliberate exception: each one you add grants its subtree outside the working folder, taking precedence over `workspace_only`. System and credential folders (`~/.ssh`, `~/.gnupg`, `~/.aws`, and OS directories) are blocked outright regardless of all three.
4. **Connect only what you need.** Every integration is a separate OAuth approval you grant (and can revoke) individually. Revoking stops the next sync; memory already collected stays in your memory engine until you forget it.

## Built-in protections you didn't have to configure

- **Prompt-injection screening.** Incoming content is screened for attempts to hijack the assistant's instructions before it acts on them.
- **Secret & PII redaction on save.** When content is written into long-lived memory, OpenHuman strips things like API keys, tokens, private-key blocks, and personal identifiers so they don't get stored.
- **Encrypted in transit.** All traffic between the app and the backend is TLS. Nothing travels in plain text.

---

## Success checks

You know your privacy posture when you can answer these:

- [ ] Is the autonomy policy on, and do you know its tier? (Check `[autonomy]` in `config.toml`.)
- [ ] If you are relying on the filesystem boundary, is `workspace_only` on as well, and do you know which trusted roots grant access outside the working folder?
- [ ] Do you know which integrations are connected? (Check **Settings**; disconnect any you don't need.)
- [ ] If locality matters for a workload, is it routed to a [local provider](local-model.md) that is running and answering?
- [ ] Are you comfortable that model turns send _retrieved snippets_, not your whole memory?

## Common misunderstandings

| Belief                                                      | Reality                                                                                |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| "My memory is stored on my laptop."                         | It is stored in CortexDB (hosted by TinyHumans, or yours). Only bookkeeping is local.  |
| "OpenHuman uploads my whole memory to the model to answer." | The model gets only what was recalled for that specific turn.                          |
| "My service passwords are on my laptop."                    | Integration tokens are held by the backend; local secrets go in your OS keychain.      |
| "Turning on local AI makes _everything_ local."             | Speech-to-text, text-to-speech, and web search still use the backend proxy by default, and memory is still stored in CortexDB. |
| "Revoking an integration deletes what it already gathered." | Already-ingested memory stays in your memory engine until you forget it; revoking only stops future syncing. |

## See also

- [Privacy & Security](../features/privacy-and-security.md): the detailed architecture.
- [Use OpenHuman with a local model](local-model.md): keep inference on-device.
- [Create a safe companion for a child](child-safe-companion.md): the strictest lockdown, composed from these controls.
