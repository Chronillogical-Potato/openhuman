---
description: >-
  Read and drive a real browser and a real desktop: accessibility snapshots,
  element references, bounded tasks that stop at a payment page, and a small
  decision model picking each step.
icon: display
---

# Browser & Computer Control

When the agent needs to _use_ a machine the way a person would, open a page, read what is on screen, click a button, type a phrase, these tools are how it does it. Both surfaces come from one loadable module, **TinyComputer**, which serves desktop and browser control over a single interface and is downloaded on first use.

## Two surfaces, one model

Desktop and browser produce the same kind of "screen": a compact list of the things a person could interact with, each with a role, a name, a value, its states, the actions it supports, and the labels of the boxes it sits in, capped at 254 controls. Both offer the same short action list (click, type, check, uncheck, expand, collapse, scroll, wait, press), so the agent learns one vocabulary.

Elements are addressed by **reference**, never by coordinates or CSS selectors. A desktop reference like `@s8f3k2p9:e1` is bound to the snapshot that produced it: acting on it either reaches the element that was described, or fails with a stale-reference error telling the agent to look again. It never clicks whatever has since moved into that position. Browser references survive a redraw, because they are marks that last as long as the element is on the page.

## Browser

The browser is a real Chrome or Chromium, driven over the Chrome DevTools Protocol by a library linked into the module. Chrome is its own process, as it has to be, but nothing sits between it and the module: no driver binary, no local HTTP bridge, no second OpenHuman process.

- **Open** a session: launch a fresh browser (headless or headed), or **attach to the Chrome you already have running**. Attaching matters more than it sounds: plenty of sites turn away a fresh automated browser and serve a person's own. On task end an attached browser is only disconnected, never closed.
- **Snapshot** the page. The default perception mode, `sight`, runs a small in-page script that reads the page the way a person sees it: real links, buttons and fields, plus anything with a pointer cursor, a click handler or a tab stop. Hidden, zero-size, transparent and disabled things are dropped; things below the fold are marked offscreen and things behind a dialog are marked covered. It falls back to the accessibility tree when it cannot read a page, and `perception: "tree"` forces the tree.
- **Act**: click (optionally into a new tab), fill, type, press, select, check, hover, scroll, wait, read, go back.
- **Read** the page as text, Markdown or serialized DOM, and **evaluate** JavaScript.
- **Downloads**: list the browser's download events, and wait for a completed file. A page snapshot alone does not prove a file finished.

Screenshots are never returned inline. A reply carries a handle the agent reads in chunks and releases when done.

Sessions are bounded: the module allows up to 8 at once, and OpenHuman keeps at most 6 per conversation thread with a 30-minute idle timeout, so a long chat reuses one browser instead of opening a new one per turn. A session can be restricted to a set of origins.

## Desktop

Desktop control reads the operating system's **accessibility tree**, the same interface a screen reader uses: running apps, windows, displays, Notification Center entries, the clipboard, and screenshots.

Actions go through the accessibility API rather than synthesized input, so by default they do **not** steal focus, move your cursor or touch the pasteboard, and a run can proceed while you use the machine. A "headed" mode and real mouse and key events exist for apps that need them, and are the exception.

For dense apps, a skeleton snapshot caps the walk at three levels and returns structure without leaf detail, which is the difference between tens of thousands of tokens and a few hundred.

## Tasks

Above the primitives sits a bounded task loop. You describe the outcome; the module plans the steps and a small decision model ([Jev](../../developing/jev.md)) picks every click. A task starts and returns immediately with a status, which the agent polls and answers:

| Status            | What it means                                                           |
| ----------------- | ----------------------------------------------------------------------- |
| `running`         | Working.                                                                |
| `needs_input`     | A field it cannot fill without you.                                     |
| `needs_approval`  | An irreversible step. Only an explicit approval releases it.            |
| `needs_human`     | A captcha, OTP, 2FA or login. No model can get past it.                 |
| `needs_plan`      | Plain language with no planner configured.                              |
| `checkpoint`      | Stopped on purpose, with the page left open.                            |
| `done` / `failed` / `cancelled` | Terminal.                                                 |

Two properties are worth stating plainly:

- **Reaching a payment page always stops the task.** It becomes a checkpoint that cannot be continued, with the page left open for a person. The agent does not pay.
- **Secret values never reach a model.** Facts you mark secret are shared as `${name}` templates; the planner, the decision model and the rescue model see the name, and the value is typed locally into the field.

Page and screen text is treated as data, never as instructions, and the safety rules are plain code rather than model judgement.

## Agent tools

| Tool | What it does |
| --- | --- |
| `browser` | Everything above on one `action` argument: `open`, `snapshot`, `read_page`, `click`, `fill`, `type`, `get_text`, `get_title`, `get_url`, `wait`, `press`, `hover`, `scroll`, `is_visible`, `find`, `task`, `task_continue`, `task_cancel`, `confirm_pending`, `list_downloads`, `wait_download`, `close`. |
| `browser_open` | Open a URL and get a session back. |
| `desktop_list_apps`, `desktop_list_windows` | What is running and where. |
| `desktop_launch` | Start an app. |
| `desktop_snapshot`, `desktop_find` | Read the screen, or look for one thing on it. |
| `desktop_goal`, `desktop_continue_goal` | Run a desktop task and answer its stops. |

Irreversible steps go through OpenHuman's [Approval Gate](../approval-gate.md), bound to a digest of the exact action and URL. If no approval gate is installed, the action is **denied**, not allowed.

## Platform support

| Platform | Desktop | Browser |
| --- | --- | --- |
| macOS | Yes. Needs **Accessibility** permission, plus **Screen Recording** for screenshots and **Automation** for the Notification Center opener. | Yes |
| Windows | Yes | Yes |
| Linux | No | No |

A permission check runs before the first read, because an accessibility API called by an unauthorized process usually returns an *empty tree* rather than an error, which is indistinguishable from an app with no buttons. A missing permission produces a named refusal that says which setting to change.

Linux is not in the shipped surface: the module registry publishes no Linux artifact, so neither desktop control nor browser automation is available there even though the browser engine itself would run.

## Settings

**Connections → Computer Control** holds all of it: the module's status and contract version, the decision model (Jev, OpenJEV or Levanto Sage) and which route it bills through, the planner and rescue models, and the browser section (executable path, perception mode, allowed domains).

`[desktop] approvals_enabled` defaults to `false`, and it governs one narrow thing: the module's own mid-goal confirmation stop on the **desktop** surface, which the core then auto-continues after the module re-observes and re-validates the exact target. Setting it `true` surfaces that stop as an approval card instead. It does not weaken the [Approval Gate](../approval-gate.md): an irreversible step still needs an approval, and with no gate installed it is denied. Desktop goal confirmations are single-use and expire after 10 minutes.

## What it's good for

- Driving sites that have no API and no [native integration](../integrations/README.md).
- Multi-step UI flows where each snapshot tells you the next actionable element.
- Automating a local app from inside a chat.

## See also

- [Web Scraper](web-scraper.md): when you only need the article, not the whole page.
- [Jev](../../developing/jev.md): the decision model that picks each step.
- [Approval Gate](../approval-gate.md): what gets parked, and what happens with no gate installed.
