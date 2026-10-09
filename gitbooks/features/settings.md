---
description: >-
  The settings surface: what each panel controls, where the autonomy tier
  actually lives, and which old addresses now redirect into Connections.
icon: gear
---

# Settings

Settings is a two-pane screen: a grouped rail on the left, one panel on the right. It holds the things that change how the app behaves. Things that change what the agent can *reach* live in [Connections](connections.md), and most of the old settings addresses for those now redirect there.

Open it with the gear in the sidebar header, or `⌘,`.

## The groups

**General**

| Panel | What it controls |
| --- | --- |
| **Account** | Who you are signed in as, your plan and usage, team, devices, notifications, and language. |
| **Privacy** | [Privacy Mode](privacy-mode.md) and what may leave the machine. |
| **Migration** | Moving your data folder, and importing from an earlier version. |
| **Core** | Point the app at a core running somewhere else: its RPC URL, a bearer token, and a live reachability check. |
| **Feedback** | Send feedback from inside the app. |
| **About** | Version, build, licences, and the capability catalogue. |

**Appearance**

| Panel | What it controls |
| --- | --- |
| **Appearance** | Font size (including a custom pixel value), layout corners and borders, and language. |
| **Theme Studio** | Theme family and variant, per-token colours, per-role fonts, backdrop, and your own saved themes. See [Themes](theming.md). |
| **Personality** | The editable `SOUL.md` prompt and its companion identity file. See [Personalization](personalization.md). |
| **Face** | Which mascot, its colours, a second "duo" mascot, a custom image, and its voice. See [The Mascot](mascot/README.md). |

**Security**

| Panel | What it controls |
| --- | --- |
| **Keychain** | The OS keyring, consent, and what is encrypted at rest. See [OS Keyring & Secret Storage](os-keyring-and-secret-storage.md). |
| **Agent access** | Trusted roots and their read or write grant, the always-allow list, the blanket auto-approve switch, action and tool timeouts, the workspace-only switch, and the tool-call dialect. |
| **Sandbox** | Which sandbox backend a session gets, and its limits. |
| **Approval history** | The audit trail of approval decisions and their outcomes. |

## Where the autonomy tier is

Not in Agent access, which is the first place everyone looks.

The tier itself (`readonly`, `supervised`, `full`) has no control in the normal settings navigation. The panel that holds the radios is registered developer-only and as a hidden deep link, so it does not appear in the sidebar. Set the tier in `config.toml`:

```toml
[autonomy]
enabled = true          # off by default; nothing below applies until this is true
level = "supervised"
```

`[autonomy] enabled` is `false` out of the box, which means Agent access's trusted roots, the always-allow list and the workspace-only switch are all inert until you turn the policy on. [Approval Gate](approval-gate.md) explains the whole model, including what stays enforced either way.

## Redirects

Roughly three times as many settings routes redirect as render, because the panels moved into [Connections](connections.md) and the addresses were kept working. Notable ones:

| Old address | Now |
| --- | --- |
| `/settings/llm`, `/settings/embeddings`, `/settings/voice`, `/settings/search` | Connections, the matching API-keys tab |
| `/settings/agents/*`, `/settings/tools` | Connections → Tools |
| `/settings/memory-*`, `/settings/intelligence` | Connections → Memory |
| `/settings/mcp-server`, `/settings/skills-runner` | Connections → MCP Servers, Connections → Skills |
| `/settings/wallet-balances` | Connections → Wallet Balances |
| `/settings/cron-jobs` | Workflows → Schedules |
| `/settings/automations` | Workflows |
| `/settings/team`, `/settings/billing`, `/settings/notifications`, `/settings/devices`, `/settings/language` | Settings → Account |

## Not here yet

There is no search across settings: the global search bar was removed, and the per-entry keywords are no longer read by anything. Notification categories gate which events are ingested, but the page that edited them is gone, so they are config-only today.

## See also

- [Connections](connections.md): everything the agent plugs into.
- [Approval Gate](approval-gate.md): the autonomy policy these panels configure.
- [Privacy & Security](privacy-and-security.md): the posture behind the privacy panel.
