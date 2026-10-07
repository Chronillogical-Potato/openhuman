---
description: >-
  Human-in-the-loop consent for side-effecting tool calls. When the autonomy
  policy is on, the agent parks any risky action until you approve it, and
  fails closed if you don't.
icon: shield-check
---

# Approval Gate

The Approval Gate is the checkpoint between the agent and the outside world. When the agent wants to run a tool with a real-world effect (post to Slack, send an email, create a calendar event, run a shell command, install a package), the gate intercepts the call, shows you what is about to happen, and waits for your decision.

{% hint style="warning" %}
**The autonomy policy is off by default, and the gate is part of it.** `[autonomy] enabled` defaults to `false` (`crates/openhuman-core/src/config/schema/autonomy.rs`). Until you turn it on, command classification, the tier table, the allowlist, the hourly action budget, `workspace_only` and `forbidden_paths` are all inert, and an acting tool call runs without a prompt.

That is deliberate rather than an oversight: these agents are expected to run inside a container, a platform jail or a Docker sandbox that already provides the isolation this in-process policy was approximating, and a shell that refuses ordinary shell syntax is not a usable shell. Set `[autonomy] enabled = true` in `config.toml` to get everything on this page.
{% endhint %}

## What still holds with the policy off

Three things do not depend on `[autonomy] enabled`:

- **Credential stores and system roots stay unreachable.** `SecurityPolicy::is_always_forbidden` refuses `~/.ssh`, `~/.gnupg`, `~/.aws` and system roots, along with `..` traversal and null bytes in a path, on every code path. It is a floor, not a tier.
- **Hard tool-level blocks are independent.** The checks inside the tool implementations themselves, including the tool-policy middleware, run regardless.
- **Sandboxing is a separate mechanism.** The platform jail and the Docker backend are chosen by `[runtime]` and the session's origin, not by the autonomy tier. See [sandboxing](privacy-and-security.md).

Everything below describes the policy with `[autonomy] enabled = true`.

---

## What triggers a prompt

Every acting tool call is classified into a **command class**, and your **autonomy tier** decides whether that class runs silently, prompts, or is blocked.

| Command class | What it covers                                                    |
| ------------- | ----------------------------------------------------------------- |
| Read          | Provably read-only or observational (curated allowlist)            |
| Write         | State-changing; the fail-closed default for anything unrecognized |
| Network       | Reaches the network (curl, wget, ssh, scp, …)                     |
| Install       | Installs an OS or global language package                         |
| Destructive   | Catastrophic, irreversible or privilege-escalating                |

The tier comes from **Settings → Permissions** (`[autonomy].level`, `AutonomyLevel` in `crates/openhuman-core/src/security/policy/types.rs`):

| Tier                   | Read  | Write  | Network / Install / Destructive |
| ---------------------- | ----- | ------ | ------------------------------- |
| `readonly`             | Allow | Block  | Block                           |
| `supervised` _(default when the policy is on)_ | Allow | Prompt | Prompt         |
| `full`                 | Allow | Allow  | Prompt                          |

Anything that lands on **Prompt** is parked at the gate. `Block` is refused outright: no in-tier approval can authorize it. Classification is fail-closed. A command that is not provably read-only is treated as at least `Write`, and across a piped command the highest class wins, so `ls | curl …` is `Network`.

A quoted heredoc body (`<< 'EOF' … EOF`) is data, not shell: it is blanked before any structural scan. An unquoted delimiter (`<< EOF`) is still expanded and still scanned.

---

## The flow

```text
agent wants to act
        │
        ▼
 classify command ──► Block ──► refused
        │
     Prompt
        │
        ▼
 on "Always allow" list? ──► yes ──► run immediately
        │ no
        ▼
 park call · persist pending row · emit approval_request
        │
        ▼
 ┌──────────────┬───────────────┬────────────┐
 ▼              ▼               ▼            ▼
Approve     Always allow      Deny      10-min TTL
(once)    (+ allowlist)                     │
 │             │               │            ▼
 ▼             ▼               ▼          Deny
 run           run           refused   (fail closed)
```

When a call is parked, an **Approval Request card** appears above the chat composer. It shows the tool name, a safe one-line summary of the action, and the redacted command. Three choices:

- **Approve**: run this one call.
- **Always allow**: run it, and add the tool to your `auto_approve` list so it skips the prompt next time.
- **Deny**: refuse this call.

You can also type **yes** or **no** in chat. The reply is routed back to the parked request.

---

## Always allow

Approving with **Always allow** persists the tool name onto `[autonomy].auto_approve` (config save plus a live policy reload), so the gate short-circuits to allow for that tool on later turns. Remove an entry in **Settings → Agent access** to start being prompted again.

The shipped default list is `file_read`, `memory_search`, `memory_list`, `get_time`, `list_dir`, `glob`, `grep`. Four of those names no longer match a registered tool: the memory surface is one tool called `memory`, the clock is `current_time`, and the directory listing is `list`. Those four entries are therefore inert, and `file_read`, `glob` and `grep` are the three that do anything. Checked against `app/src/features/conversations/tools/__fixtures__/coreToolNames.json`. That fixture is the catalogue the core's drift test compares against, though it is not exhaustive: a few credential-gated and env-gated tools are registered without appearing in it.

---

## `auto_approve_all`

Separately from the per-tool list, `[autonomy].auto_approve_all` approves **every** call without prompting. Two things are worth knowing before enabling it:

- An unlabelled call site is still hard-denied, and the hard security blocks inside the tool implementations are unaffected.
- It bypasses **parking**, not just the prompt. A remote-origin triage dispatch (a connector or webhook payload reaching `triage.escalate`) normally parks and writes an audit row; with this flag on it is allowed immediately and **no audit row is written**, so those dispatches leave no approval trail.

---

## Fail-closed behavior

Every non-approve path resolves to **Deny**:

- **Timeout**: a parked request lives for 10 minutes; if undecided it transitions to a terminal `deny`.
- **Persist failure** or a dropped channel: denied.
- The timeout path re-reads the stored decision first, so an approval that committed in the race still wins.

Pending requests are stored in SQLite (`{workspace_dir}/approval/approval.db`) and **survive a core restart**. After an approved tool finishes, the gate records a write-once execution outcome (success or error, with the error text sanitized and capped) as a durable audit trail. Everything persisted or broadcast is redacted first: PII and chat content are scrubbed and home paths stripped.

---

## Background and cron bypass

The gate is **interactive-only**. Background, triage and cron turns carry no chat context, so there is nobody to answer a prompt. These turns are pre-authorized and pass straight through, with no row and no event. Approval is enforced for live chat turns.

---

## Configuration and RPC

- **`[autonomy].enabled`**: the master switch. `false` by default; everything on this page needs it `true`.
- **`OPENHUMAN_APPROVAL_GATE`**: set to `0` or `false` to skip installing the gate even with the policy on. With no gate, `Prompt`-class calls run unprompted.
- **`[autonomy].level`** and **`[autonomy].auto_approve`**: tier and allowlist, via the `config.update_autonomy_settings` RPC or the settings panels.

The `approval` controller exposes three JSON-RPC methods:

| Method                                     | Purpose                                                                                                  |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| `openhuman.approval_list_pending`          | The live queue of parked requests.                                                                       |
| `openhuman.approval_list_recent_decisions` | Decided and executed audit rows (`limit` 1 to 500, default 50). Surfaced in **Settings → Approval history**. |
| `openhuman.approval_decide`                | Apply a decision (`approve_once` / `approve_always_for_tool` / `deny`).                                  |

`list_pending` and `list_recent_decisions` return empty rather than an error when no gate is installed; `decide` errors when the gate is absent or the request is unknown or already decided.

---

## See also

- [Privacy & Security](privacy-and-security.md): what leaves the machine, and the layers underneath this one.
- [Security architecture](../developing/architecture/security.md): command classification and policy internals.
- [Agent access settings](settings.md): where the tier, the trusted roots and the allowlist are edited.
