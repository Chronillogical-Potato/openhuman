---
description: >-
  The notification center, the quiet notice tray, and where background work
  now surfaces: everything OpenHuman tells you about, and everything it does
  while you are not watching.
icon: bell
---

# Notifications & Activity

OpenHuman surfaces two kinds of "what's happening": **notifications** (things you should look at, like an important Slack message, a failed webhook, or a high-priority email) and **activity** (a ledger of what the agent did on its own while you weren't watching).

Both live on the **Notifications** page. There is no longer an Activity hub or a Routines screen. Both addresses still resolve, but not to what they used to show: `/activity` redirects to **Settings → Account**, and `/routines` to [Workflows](workflows.md). The notification feeds moved here, and the scheduler moved to Workflows → Schedules.

---

## Notification Center

The notification center is fed by two independent streams, which render as two stacked sections on the Notifications page.

### Integration notifications

Notifications captured from connected accounts (Gmail, Slack, WhatsApp, Discord, …) are ingested through the `notification.ingest` RPC, persisted to a per-workspace SQLite store, and then **triaged by a local LLM in the background**. Ingest returns immediately; triage runs in a spawned task and back-fills the score a moment later, so a freshly arrived item can briefly show as unscored.

Triage assigns each notification an **action**, which maps to a fixed importance score between 0.0 and 1.0:

| Triage action | Score | What it means                    |
| ------------- | ----- | -------------------------------- |
| `drop`        | 0.10  | Noise, not worth surfacing       |
| `acknowledge` | 0.35  | Low value, informational         |
| `react`       | 0.65  | Worth a follow-up                |
| `escalate`    | 0.90  | High priority, hand to the agent |

Only `react` and `escalate` are considered "routed" actions; `drop` and `acknowledge` stay quiet. Each ingested item carries a one-sentence `triage_reason` justifying the classifier's call, plus a lifecycle status: **unread → read → acted → dismissed**. Duplicate content received within a 60-second window collapses to a single entry.

### System (core-bridge) notifications

The second stream translates selected internal events into compact, user-facing alerts and pushes them over the socket bridge as they happen. These are persisted before broadcast, so anything fired while the app was closed syncs down on the next open. Each carries a **category** and an in-app deep link:

| Source event         | Category | Surfaces when                             |
| -------------------- | -------- | ----------------------------------------- |
| Cron job completed   | Agents   | Always (success or failure)               |
| Webhook processed    | System   | **Only on failure**; successes are silent |
| Sub-agent finished   | Agents   | Always                                    |
| Sub-agent failed     | Agents   | Always                                    |
| Notification triaged | Agents   | Only when routed (`escalate`/`react`)     |
| API key rejected     | System   | Always; links to the LLM settings tab     |

The category set the notification center understands is **messages, agents, skills, system, meetings, reminders, important**. The page shows a filter chip row, but only for the categories that actually appear in the current feed, plus **Mark all read** and **Clear**. Clicking a notification marks it read and follows its deep link. Some core notifications carry **action buttons** and are pinned to the top. The feed holds the most recent 200 items.

### Per-provider routing & thresholds

Every provider has its own settings (`notification.settings_set`), letting you tune the noise per source:

| Setting                 | Effect                                                                         |
| ----------------------- | ------------------------------------------------------------------------------ |
| `enabled`               | When off, that provider's notifications are not ingested at all                |
| `importance_threshold`  | Minimum score (0.0 to 1.0) to display; `0.0` shows everything                  |
| `route_to_orchestrator` | When on, high-importance (`react`/`escalate`) items are forwarded to the agent |

Auto-routing re-reads the provider's settings the moment before escalating, so toggling a setting mid-flight takes effect immediately. A notification is only routed to the agent when its score clears the provider threshold **and** `route_to_orchestrator` is enabled.

These per-provider settings have no UI today: the page that edited them was removed, so they are set through the RPC or by hand. The per-category preferences gate **ingest**, not just display, which is worth knowing before changing one.

---

## The notice tray

Separate from the notification center, a quiet tray in the bottom-right corner collects **things you can act on**: a provider key that was rejected, a plan limit reached, a keyring consent prompt. It replaced the full-width banners that used to push the chat down. Repeats of the same problem bump a count rather than stacking, and the tray is in-memory, so it clears on restart.

Product announcements arrive separately again, as a modal shown once per signed-in session, with the ids you have already seen remembered.

---

## Where background work surfaces

| Kind of background work | Where to look |
| --- | --- |
| Scheduled jobs and their run history | **Workflows → Schedules**. See [Cron & Scheduling](native-tools/cron.md). |
| Workflow runs | **Workflows → Runs** |
| Memory belief builds and source syncs | **Connections → Memory → Background** |
| Detached sub-agents and async delegation | The background inbox card in the thread that started them. See [Chat](chat.md). |
| Everything above, as notifications | The Notifications page, Agents category |

---

## See also

- [Cron & Scheduling](native-tools/cron.md): the scheduling engine and the agent tools behind the Schedules view.
- [Triggers](integrations/triggers.md): webhooks and inbound events that can raise a notification.
- [Chat](chat.md): where an approval or a sub-agent result lands when you are in the conversation.
