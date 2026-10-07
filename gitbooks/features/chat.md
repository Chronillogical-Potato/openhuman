---
description: >-
  The chat surface: composer, slash commands, mentions, model and effort
  pickers, the context ring, the tool timeline, approvals, sub-agent cards,
  artifacts and threads.
icon: comments
---

# Chat

Chat is where almost everything happens. It is not a text box with a send button: a turn here can call tools, spawn sub-agents, stop to ask you something, park an action for approval, produce files, and bill itself, and the transcript shows all of it in order.

Open it at **Chat** in the sidebar, or `⌘N` for a new conversation.

## The composer

The input is a rich text field rather than a plain one, which is what lets a `/` become a command chip and a popover anchor to the caret as you type. Alongside it:

- **Attach** files. An image, a PDF, an Office document or an archive is preserved in the working folder and offered to the model natively when the model and the route both support that type; otherwise it degrades to extracted text, a bounded readout, or an archive listing with accessible paths.
- **Model**, and a separate **thinking effort**. Both apply to this conversation, not globally.
- **Context ring**, which shows how full the context window is for this thread and opens a breakdown with per-sub-agent cost.
- **Voice**: tap to speak, or hand the turn to the live voice agent. See [Voice](native-tools/voice.md).
- When the box is empty, the mascot button takes you to the full-screen [mascot](mascot/README.md) stage.

### Slash commands

Typing `/` offers three sources at once: the built-ins (`/new`, `/clear`, `/stop`, `/plan`, `/build`), the core's own command list, and your installed skills and workflows.

### Mentions

Typing `@` offers two sources: **memory**, searched as you type, and **files**, meaning the artifacts this thread has already produced. A mention becomes a chip, so the agent gets the reference rather than a guess at what you meant.

## Reading a turn

Each assistant turn groups its reasoning and its tool calls into one chronological block, so you can see what it thought and what it did in the order it happened rather than in two separate panes.

- **Tool calls** render per tool: a search shows its queries and sources, a shell call shows its command and exit, a failed call shows a failure card with the reason rather than a stack trace. Every tool runs in the core; nothing executes in the browser.
- **Sources** appear as badges under an answer, up to four inline and then a count.
- **Timing and cost** are on the turn, and the context ring's breakdown attributes spend to the sub-agents that caused it.
- **Find in conversation** (`⌘F`) searches this thread only, and the right-edge rail jumps between turns.

The action bar on an assistant turn has copy, regenerate, a thumbs up or down that is remembered, read-aloud, and export as Markdown. On your own turns you can edit and resend, which branches the conversation: the branch picker moves between versions.

## When the agent needs you

Four different kinds of stop, all inline:

| Stop | What it looks like |
| --- | --- |
| **Approval** | A card naming the tool and a redacted one-line summary of the action. Approve once, always allow that tool, or deny. See [Approval Gate](approval-gate.md). |
| **Permission** | A connector or provider needs authorizing; the card collects exactly the fields that provider needs. |
| **Clarification** | The agent asks a direct question and waits. |
| **Plan review** | A proposed plan with approve, reject, or revise-with-feedback. |

A background or scheduled turn has nobody to ask, so an approval raised outside a live chat collects in a deck you can decide later. Undecided requests deny themselves after ten minutes.

## Sub-agents

When a turn delegates, each child appears as its own card: a nested transcript, its own status, a reply box when it is waiting on you, and a cancel button. Delegation is asynchronous, so the parent keeps working and the card fills in as the child reports. Detached background agents, scheduled runs and memory syncs collect in a separate inbox card rather than interrupting the thread.

## Files the agent makes

Anything the agent produces that is meant for you, a document, a deck, an image, a render, becomes an artifact: a chip above the composer opens a panel with download, reveal in folder, and delete. Only finished artifacts persist across a restart; a failed one offers a retry.

## Workflow proposals

Ask for an automation and the agent proposes a graph. The proposal is validate-only: it arrives as a card with the node list and three choices, save and enable, dismiss, or open it on the canvas. That card is the only path from a proposal to a saved automation, which is deliberate. See [Workflows](workflows.md).

## Threads

The sidebar lists your conversations. You can create, select, rename inline, and delete behind a confirmation. Turns on different threads run **concurrently**, so starting something slow in one conversation does not block another.

Threads carry a goal and a to-do list when the agent sets them, shown as a pinned list and a banner with its token budget. Both are read-only in the UI today: the agent maintains them, you do not edit them there.

A long conversation is compacted rather than truncated. The compaction seals what came before and opens a new generation that records the sealed one as its parent, so nothing is deleted on disk even though the model only reads the head. There is no UI for browsing those generations yet.

## Keyboard

| Shortcut | Action |
| --- | --- |
| `⌘K` / `⌘P` | Command palette |
| `⌘N` | New conversation |
| `⌘F` | Find in this conversation |
| `⌘B` | Toggle the sidebar |
| `⌘,` | Settings |
| `⌘/` or `?` | Shortcut sheet |

## Not here yet

Stated so you don't go looking: there is no thread search, pinning, folders or archiving in the sidebar; no UI for setting a thread goal or ticking a to-do; no browser for compaction generations; and follow-up suggestion chips are built but have no producer, so the row stays empty.

## See also

- [Memory](memory.md): what the agent recalls before it answers, and what it writes after.
- [The Orchestrator](orchestration.md): how delegation is planned.
- [Available Tools](native-tools/README.md): what those tool cards are calling.
- [Notifications & Activity](notifications-and-activity.md): what happens when a turn finishes while you are elsewhere.
