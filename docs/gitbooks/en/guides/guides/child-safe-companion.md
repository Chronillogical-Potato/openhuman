---
description: >-
  Compose OpenHuman's autonomy, approval, and content-screening controls into a
  locked-down assistant for a child, with an honest account of the limits.
icon: child
---

# Create a safe companion for a child

**Goal:** set up the most restricted, supervised version of OpenHuman you can, intended for a child to use with an adult present.

{% hint style="danger" %}
**Read this first. It is an honest warning.** OpenHuman has **no dedicated "child mode"**, no age verification, and no content filter on what the model _says_. There is no certified parental-control product here. What you _can_ do is compose the existing safety controls into a tightly locked-down setup. That reduces risk; it does not make an AI assistant a safe, unsupervised experience for a child. **Adult supervision is the control that matters most.** Do not rely on software alone.
{% endhint %}

This guide is about stacking the real controls that exist, and being clear about what they do and don't cover.

***

## Prerequisites

* OpenHuman set up on the machine the child will use. See [Create my personal AI assistant](personal-assistant.md).
* An adult who owns the account and stays involved.

## Privacy implications

* Keep it local. Use a [local model](local-model.md) so conversations aren't sent to a cloud provider, and connect **no** personal integrations.
* Do not connect a child's accounts. The safest memory is minimal memory.

## What each control actually does

| Control                                       | What it protects against                                                                                       | What it does **not** do                        |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| **`readonly` autonomy** (needs the policy on) | The assistant taking any action (sending, writing files, running commands, or reaching the network on its own) | Doesn't filter what it _says_                  |
| **Approval Gate (on)**                        | Any state-changing/network action slipping through without an adult's yes                                      | Doesn't review conversation content            |
| **Workspace-only + blocked system dirs**      | The agent touching files outside a small folder, or any credential/system directory                            | Doesn't restrict what topics come up           |
| **Prompt-injection screening**                | Attempts (in pasted text) to hijack the assistant's instructions                                               | Isn't a general content moderator              |
| **No integrations connected**                 | The assistant pulling in or acting on personal data                                                            | (Nothing; not connecting is the whole control) |

The honest gap: **none of these filter the model's language or subject matter.** That gap is filled by an adult in the room and by persona instructions, not by a setting.

***

## Steps

### 1. Set the strictest autonomy tier

**Settings → Agents → Agent access:**

* Autonomy: `[autonomy] enabled = true` with `level = "readonly"` in `config.toml`. The assistant can then talk and answer, but cannot act, write files, or reach the network on its own. The policy is off by default, so this is the first thing to set.
* Keep **workspace-only** on.
* Leave the [Approval Gate](../../features/approval-gate.md) installed as a second layer. (At `readonly`, acting is blocked outright anyway.)
* Review the **auto-approve** list and remove anything you don't want running without a prompt.

### 2. Keep inference and data local

* Set up a [local model](local-model.md) (run the runtime and pull the model yourself), then **route chat and reasoning at the local provider** so conversations run on-device. Adding the provider alone does not move chat; it stays on the default cloud route until you point the chat/reasoning workloads at the local provider and confirm with a test message. Turning on `local_only` [Privacy Mode](../../features/privacy-and-security/privacy-mode.md) makes that guarantee hard.
* Connect **no** integrations. Don't sign the child's accounts in.

### 3. Write a protective persona

Edit the behavior prompt (`SOUL.md`, via the **Brain** page `/brain`) to set age-appropriate rules directly. For example: "You are talking with a child. Keep language simple and kind. Refuse and redirect anything violent, sexual, frightening, or unsafe. Never give instructions that could cause harm. Encourage them to ask a parent." Persona instructions are your main lever over _content_, since there's no built-in filter.

### 4. Supervise, and test first

* Sit with the child, at least at first.
* Before handing it over, **try to break it yourself**: ask it things a child might, and confirm the persona redirects appropriately.

***

## Success checks

* [ ] `[autonomy] enabled = true` and `level = "readonly"`; `workspace_only` is on.
* [ ] No integrations are connected.
* [ ] Inference is local (`ready`), so conversations aren't going to a cloud provider.
* [ ] In your own testing, the persona refuses and redirects unsafe prompts.
* [ ] An adult is present for use. _(This is a check, not a nicety.)_

## Common failures

| Symptom                                        | Cause                                                      | Fix                                                                              |
| ---------------------------------------------- | ---------------------------------------------------------- | -------------------------------------------------------------------------------- |
| It produced content you consider inappropriate | There is no content filter; the persona alone governs tone | Strengthen the `SOUL.md` rules; supervise; this is an inherent limit of the tool |
| It tried to do something (send/open/fetch)     | The policy is off, or the tier isn't `readonly`            | Set `enabled = true` and `level = "readonly"` in `config.toml`                   |
| Conversation went to the cloud                 | Chat isn't routed to a local provider                      | Route chat to a [local model](local-model.md) and send a test message            |
| The child reached settings and changed things  | OpenHuman has no separate child login                      | Use OS-level user accounts/parental controls to lock down the machine itself     |

## Recovery

* **Instant lockdown:** set `enabled = true` and `level = "readonly"` (if either drifted). The level does nothing while `enabled` is `false`, so check both. Acting stops next turn.
* **Reset persona:** revert your `SOUL.md` edits to defaults if the customization misbehaves.
* **The real recovery is supervision.** If the experience isn't right for the child, step in. No software setting substitutes for that.

## See also

* [Keep sensitive data private](privacy-sensitive-data.md): the controls this guide stacks.
* [Approval Gate](../../features/approval-gate.md): the autonomy tiers and what each one blocks.
* [Privacy & Security](../../features/privacy-and-security/): what leaves the machine, and what does not.
