---
description: >-
  Let OpenHuman tidy, rename, and restructure a folder of files safely, inside a
  boundary you set, with changes gated by your approval once the autonomy policy
  is on at the supervised tier.
icon: folder-tree
---

# Organize my project folders

**Goal:** point the assistant at a folder and have it clean it up (sort files, rename consistently, remove clutter) without letting it roam your whole disk or make changes you didn't see.

The core idea: the agent works inside a **boundary you define**, and with the autonomy policy on at the `supervised` tier, any file change that isn't provably read-only is **parked for your approval** before it runs. The tier is what decides that: at `full`, routine writes run on their own and only network, install and destructive actions stop to ask.

***

## Prerequisites

* OpenHuman set up. See [Create my personal AI assistant](personal-assistant.md).
* A specific folder you want organized. Ideally, make a copy first if the contents are irreplaceable.

## Privacy implications

* File organizing is **local**. Reading, moving, and renaming files happens on your machine.
* If you ask the agent to _reason about_ file contents (e.g. "group these by topic"), it may send relevant snippets to the model to do that. Route inference to a [local model](local-model.md) if you want that reasoning on-device too.
* The agent cannot touch system or credential folders (`~/.ssh`, `~/.gnupg`, `~/.aws`, OS directories). Those are blocked outright regardless of settings.

***

## Steps

### 1. Decide where the agent may act

{% hint style="warning" %}
Everything in this step needs the autonomy policy switched on. It is **off by default**: set `[autonomy] enabled = true` in `config.toml` (`~/.openhuman/config.toml`, or `%USERPROFILE%\.openhuman\config.toml` on Windows). Without it, trusted roots, `workspace_only` and the approval gate are all inert and acting tool calls run unprompted. Credential stores and system roots stay blocked either way.
{% endhint %}

Confinement to the working folder has two preconditions: the policy enabled, and `workspace_only` on. With both, the agent's read/write root is its working folder and it has no ambient access to the rest of your disk. With either one off, that boundary is not enforced.

A **trusted root** is the deliberate exception. Each one grants its subtree even though it sits outside the working folder, taking precedence over `workspace_only`. To let the agent work on a folder elsewhere, add that folder as a trusted root:

* Open **Settings → Agent access**.
* Add the target folder as a trusted root with read-write access.

Keep the boundary as tight as the task: grant the one folder, not your home directory.

### 2. Set the right autonomy tier

In the same `[autonomy]` block, `level` decides how much runs without asking:

* `supervised` _(recommended)_: the agent proposes each change and you approve moves, renames and deletes as they come, except where you have already put the tool on the always-allow list.
* `full`: routine file writes run automatically. Still tighten the trusted root so "automatic" stays contained.

Deleting and moving files are state-changing actions, so with `supervised` they are parked for your yes or no by the [Approval Gate](../../features/approval-gate.md). Answering **Always allow** to a prompt adds that tool to the always-allow list, and from then on its calls run without a fresh approval. Remove it from the list in **Settings → Agent access** when you want each call reviewed again.

### 3. Ask for the reorganization

Be concrete about the folder and the rules. For example:

* "In my trusted `~/Documents/receipts` folder, rename every file to `YYYY-MM-DD-vendor.pdf` based on its contents, and move anything older than 2023 into an `archive/` subfolder."
* "Group the loose files in this folder into subfolders by type, and show me the plan before doing anything."

### 4. Review each proposed action

When the agent wants to move, rename, or delete, an **Approval Request card** appears with the exact action. **Approve** one, **Always allow** a repetitive safe one, or **Deny**. You can also just type **yes** / **no**.

***

## Success checks

* [ ] The policy is enabled and `workspace_only` is on, so the folder boundary is actually being enforced.
* [ ] The agent only touched the folder you granted, and nothing outside your working folder and trusted roots changed.
* [ ] Each move/rename/delete showed up as an approval prompt (unless you chose "Always allow" for that tool).
* [ ] The folder matches the structure you asked for.
* [ ] Files you didn't mention are untouched.

## Common failures

| Symptom                                   | Cause                                                                       | Fix                                                                                               |
| ----------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| "I can't access that folder"              | The folder isn't a trusted root, or `workspace_only` is confining the agent | Add it in **Settings → Agent access** as a read-write trusted root                                |
| It asks for approval on every single file | The `supervised` tier gates each write                                      | Use **Always allow** for the specific safe tool, or narrow the request so there are fewer actions |
| It refuses to touch a path                | The path is a blocked system/credential directory                           | That's by design; those paths are never accessible. Choose a normal working folder                |
| It reorganized more than you wanted       | The instruction was broad                                                   | Ask it to "show the plan first"; approve selectively                                              |

## Recovery

* **Nothing runs without approval** at the `supervised` tier, apart from the tools on your always-allow list. If a plan looks wrong, **Deny** and it doesn't happen.
* **Undo is manual.** OpenHuman doesn't roll file operations back for you, so work on a **copy** of anything precious, or keep the folder under version control (e.g. `git`) so you can revert.
* If the agent is doing too much, set both `enabled = true` and `level = "readonly"`. The level does nothing on its own: with `enabled = false` the whole policy is inert. With both set, the agent can still suggest a plan but can't change files.

## See also

* [Approval Gate](../../features/approval-gate.md): exactly what gets parked and why.
* [Coder toolset](../../features/native-tools/coder.md): the filesystem/git tools the agent uses here.
* [Privacy & Security](../../features/privacy-and-security/): workspace scoping and path hardening.
