# Memory in OpenHuman: the agent lifecycle

Status: accepted. OpenHuman drives TinyMemory's agent memory lifecycle
(`tinymemory_tools::AgentMemory`, `vendor/tinymemory/docs/specs/agent-memory.md`
and `vendor/tinymemory/docs/integration.md`) around every agent turn. It
replaces memory v2's own plumbing: per-thread conversation batching with an
idle flusher and an exit flush, the cron-compiled `context.md` injected on a
new thread's first message, and the nested per-agent namespace tree. The v1
surface (the `memory_tree`, `memory_goals`, `people`, `tree_summarizer`,
`slack_memory` and `memory_sync` namespaces) was removed before that.

The engine contract (`MemoryEngine`: recall, fetch, store, forget, list,
consolidate) lives in `tinymemory-api`; the lifecycle, the layout, the brain
and holistic recall in `tinymemory-tools`; the CortexDB engines, document
conversion, source readers, scrubbing and the v1 importer in
`tinymemory-integrations`. OpenHuman is the host: it binds an engine, decides
who is acting, calls the hooks, queues the slow work and exposes RPC and UI.

## Model

| Concept | Meaning |
| --- | --- |
| Engine | Who stores and answers: `tinyhumans` (CortexDB behind the TinyHumans backend, needs sign-in or an API key) or `cortexdb` (your own CortexDB, endpoint + key). |
| Layout | TinyMemory's standard tree below one root: the **brain** (`source:<kind>` nodes, documents every agent shares), each **agent's conversations** (`agent:<id>`, one item per turn), and shared **learnings** at the root. One root per tenant. |
| Memory pack | A token-budgeted markdown block recalled for one turn: learnings (and the beliefs the engine built), brain documents, this agent's history, other agents' conversations. |
| Belief build | Consolidation of stored items into beliefs, run by the engine. Queued as a background job, run minutes behind the writes. |

With no usable engine (signed out and no CortexDB key), memory is **off**:
- the `memory` tool is not registered;
- the hooks do nothing and the turn runs as usual;
- RPCs answer `MEMORY_OFF`;
- the UI explains why.

## Who is acting (`memory::scope`)

A `MemoryIdentity` (an agent definition and, for a team member, its team) is
scoped around every turn: by the session host and channel dispatch for
top-level agents, the sub-agent runner for children, the team runtime for
members. Memory resolves it against the config of whoever reads or writes,
first match wins:

| | memory agent id | layout root |
| --- | --- | --- |
| 1. host binding | `[memory] agent_id` | `[memory] root` |
| 2. definition pin | `[memory.agents.<definition>] agent_id` | `[memory.agents.<definition>] root` |
| 3. team member | the definition id | `team:<team>` |
| 4. default | the definition id (`assistant` outside an agent) | the default root |

The host binding is how a coordinating host reuses OpenHuman agents with
memory of their own: an embedder sets it per agent with
`openhuman_embed::AgentSpec::memory(MemoryBinding::new(id).root(root))`,
which lands on the agent's derived config. Everything that agent runs,
sub-agents included, acts as that one memory agent. A sub-agent otherwise
acts as its own definition id under its parent's root. The identity is never
taken from model arguments.

A host managing several tenants reads and manages each one's memory with
`openhuman_embed::Runtime::memory(root)`. The facade (`openhuman_embed::memory`)
is bound to one root, and every call it makes stays inside that root's subtree:
- reads carry `Reach::subtree(root)`;
- `forget` only removes ids found there;
- `learn` writes at the root whatever metadata it is given.

Its calls are `agents`, `list` (whole root or one agent's node), `get`,
`recall`, `fetch`, `learn`, `forget`, `forget_agent` and the brain calls. It
does not use the ambient-config RPCs.

`RuntimeBuilder::memory_engine` (or `memory::engine::install_host_engine`)
binds a host-supplied `MemoryEngine` for the whole process, ahead of the
configured one. A host that owns its store, or a test using TinyMemory's
in-memory `ReferenceEngine`, then runs the full lifecycle without a TinyHumans
credential.

## The lifecycle

| Hook | Where | What |
| --- | --- | --- |
| pre-turn | session host `before_turn` (`agent/session_host/runtime_session/memory_ingest.rs`), concurrently with request enrichment | `AgentMemory::pre_turn`: logs the user turn (accepted, not indexed) and recalls the turn's pack, under `[memory.recall] pre_turn_timeout_ms` |
| inject | `MemoryPackMiddleware` (`agent/tinyagents/middleware/memory_pack.rs`) | adds the pack, wrapped in `<memory-context>`, to every model request of the turn with `push_ephemeral_instruction` |
| post-turn | session host, after the durable commit | `AgentMemory::post_turn`: logs the reply with its tool calls (name, id, and a one-line result per call), queues the belief build it hands back |
| compaction | `MemoryRecallSummarizer` (`agent/tinyagents/memory_summarizer.rs`) wrapping the turn's summarizer | `AgentMemory::recall_for_compaction` over the dropped turns, concurrently with the summary; appended to the checkpoint under "Recalled from memory" |
| session start | pre-turn, on the first turn of a session resumed after a compaction | `AgentMemory::start_session`: the thread's own earlier turns lead the pack |

- **Turn indices** follow the committed transcript: the user message after
  `n` committed turns is `2n`, its reply `2n + 1`. They are stable across
  retries and restarts, so a retried turn is a replay, not a duplicate.
- **The pack never enters the transcript.** It rides a request-only copy, so
  the thread's persisted history and the provider's cached prefix are exactly
  as they were. The window of the thread still verbatim in the prompt (from
  the turn after the last compaction checkpoint) is left out of the pack.
- **No hook fails a turn.** Each is bounded and logs instead of raising; a
  timed-out pre-turn still finishes logging in the background.
- **A refused recall is not an empty one.** When nothing was recalled because
  the engine refused the account (`INSUFFICIENT_CREDITS`, `UNAUTHORIZED`,
  `UNAVAILABLE`), the turn is given a short notice saying memory is
  unavailable and why, so the model does not tell the user nothing is stored.
  TinyMemory's holistic recall reports a section's failure as a skip reason,
  so the hook reads the refusal back from there too.
- **Every write is scrubbed.** The bound engine is wrapped in
  `memory::guard::ScrubbingEngine`, so turns, documents and learnings are
  scrubbed of secrets and PII whichever path writes them.
- `omit_memory_context` on an agent definition turns its pack off; its turns
  are still logged.
- The pack's cited items are recorded as the turn's memory citations, so the
  chat shows what memory the answer drew on.

## Background jobs (`memory::lifecycle::jobs`)

`post_turn` and brain ingests hand back `BackgroundJob`s. They are persisted
in `<workspace>/memory/jobs.json`, merged when identical, and run by the
`memory_background` system cron job (every 5 minutes) once they are
`[memory.recall] build_delay_secs` old, since a build reads the facts the
engine extracts from the writes. Background work pauses with the scheduler
gate. A failing job is retried up to five times; an account-wide refusal
(no credits, a rejected credential, an unreachable engine) does not count as
an attempt, so the job waits instead of being dropped. The run that drops a
job records why. The hosted engine builds on
its own schedule (`scheduled`); an engine that cannot consolidate answers
`skipped`.

## The brain (`memory::brain`, `memory::sources`)

- **Synced sources** (`folder`, `file`, `link`, `github`, `rss`)
  are filed under the brain source their items belong to: GitHub → `github`,
  links and feeds → `web`, local files by type (`pdf`, HTML → `web`, other text →
  `markdown`). A source's `namespace` names the layout root it files under.
- **Ingest** (`memory_brain_ingest`) files a local file (converted, its
  source picked from its format) or text, accepted without waiting for
  indexing. Files are capped at 25 MB (`MAX_INGEST_BYTES`).
- **A long document is several writes.** CortexDB refuses an event over
  1 MiB, so tinymemory writes a document whose text is over about 256 KiB as
  pieces of about 256 KiB each, cut at page breaks and headings (PDF pages
  are joined with a form feed, U+000C, which the scrubber keeps). Each piece
  is one write, and the hosted engine bills per write: a 25 MB text file is
  about 100 billed writes. It still reads back as one item, and a search hit
  on it is the matching piece, tagged `page:`/`section:`.
- Each ingest or sync queues one belief build per brain source it touched,
  unless the engine rebuilds beliefs on its own (`Consolidation::Automatic`),
  which needs none.

## Config (`config.toml`)

```toml
[memory]
engine = "tinyhumans"            # "tinyhumans" | "cortexdb"
# agent_id = "employee-7"        # host binding (see "Who is acting")
# root = "team:acme"
# observed_actor = false         # attribute writes to who said or did them (cortexdb only)

[memory.engines.cortexdb]
endpoint = "https://api-v1.cortexdb.ai"   # key in the keychain as "memory-cortexdb"

[memory.conversations]
enabled = true                   # log every turn

[memory.recall]
enabled = true                   # give every turn a pack
budget_tokens = 1200
learnings_limit = 8
brain_limit = 6
history_limit = 6
team_limit = 3                   # 0 leaves other agents' turns out
build_beliefs_every = 10         # turns between belief builds; 0 = off
pre_turn_timeout_ms = 1500
compaction_timeout_ms = 8000
build_delay_secs = 300

[memory.agents.researcher]
agent_id = "research-desk"
root = "project:q4"
recall = false
```

Retired keys (`[memory.context]`, `[memory.conversations] batch_turns` and
`idle_secs`, `root_agents`, `[memory.agents.<id>] namespace / inherit /
context`) and v1 keys are ignored. The retired `memory_context_refresh` cron
row is removed at startup.

The hosted engine sends the installed transport's attribution headers
(`x-sdk-name`, …) on every request; the CortexDB engine, a third party, does
not.

`observed_actor` (off by default) makes the CortexDB engine name who said or
did what it stores (CortexDB's `observed_actor`, with the memory's owner as
`subject`): an assistant turn its agent (`agent:<id>`), a synced email its
sender (`user:<address>`, with the sender's name). An email address is
trimmed and lower-cased, so one person is one actor; a phone number is kept as
the connector's dial digits (`+15551234567`). Neither is redacted. A user
turn whose origin is a channel message is observed from its sender
(`memory::lifecycle::sender`): `user:+<dial digits>` for a phone on a
phone-addressed channel, `user:<email>`, else `<channel>:<sender>` (a Telegram
id, a WhatsApp `<lid>@lid`), with the channel's push or profile name (no
channel exposes a saved contact name); a cron run is not attributed. Today
the channel runtime runs its turns on the harness graph, which logs no turns
(`memory::bus`), so this applies once a channel turn is logged. A write
CortexDB refuses for it is written again without it (tinymemory's fallback). Off, and always on the hosted engine, nothing on the wire changes.

## Agent tool: `memory`

One tool, `action` = `recall` | `fetch` | `learn` | `forget`:
- `recall { question, filter? }` → `{answer, citations[]}`.
- `fetch { query, mode?, filter?, limit? }` → `{hits[]}`; `mode` limited to the engine's `fetch_modes`.
- `learn { text, kind?, confidence? }` → `{id}`. The host fills `meta`: the
  layout's learnings node, `workspace`, `thread_id`, `agent_id` (the memory
  agent id), `tool_call`, `source.kind = "agent"`.
- `forget { ids }` → `{forgotten}`.

Every read and `forget` is confined to the identity's layout (its root's
subtree); a `reach` in the model's filter is overwritten.

## RPC (`openhuman.memory_*`)

Errors use the standard structured error; `code` is one of `MEMORY_OFF`,
`UNSUPPORTED`, `INVALID_REQUEST`, `UNAUTHORIZED`, `INSUFFICIENT_CREDITS` (the
hosted engine's 402), `UNAVAILABLE` (unreachable, timed out or overloaded) or
`ENGINE`. The two account-wide refusals have their own codes so the UI can
say "top up" or "try again" instead of showing an engine fault or an empty
memory.

| Method | Params | Result |
| --- | --- | --- |
| `memory_engines_list` | `{}` | `{engines: EngineDescriptor[], active: string\|null}` |
| `memory_engine_get` | `{}` | `{engine, endpoint?, has_key, status: "ok"\|"degraded"\|"down"\|"off", reason?, fetch_modes}` |
| `memory_engine_set` | `{engine, endpoint?, api_key?}` | same as `engine_get` |
| `memory_policy_get` | `{}` | `{log_conversations, recall: {enabled, budget_tokens, learnings_limit, brain_limit, history_limit, team_limit, build_beliefs_every, pre_turn_timeout_ms, compaction_timeout_ms, build_delay_secs}, root, agent_id, host_bound}` |
| `memory_policy_set` | `{log_conversations?, recall_enabled?, budget_tokens?, learnings_limit?, brain_limit?, history_limit?, team_limit?, build_beliefs_every?, pre_turn_timeout_ms?}` | same as `policy_get`; unknown keys refused |
| `memory_pack_preview` | `{query?, thread_id?, agent_id?}` | `{agent_id, root, mode: "turn"\|"session", pack: {markdown, tokens, refs, sections, skipped, engine}}`; reads only |
| `memory_recall` | `{question, filter?, limit?}` | `{answer, citations: Citation[], model?}` |
| `memory_fetch` | `{query, mode?, filter?, limit?, cursor?}` | `{hits: Hit[], next_cursor?}` |
| `memory_learn` | `{text, kind?, confidence?, meta?}` | `{id}` |
| `memory_forget` | `{ids, reach?}` | `{forgotten}` |
| `memory_items_list` | `{filter?, limit?, cursor?, path?}` | `{items: Hit[], next_cursor?}` |
| `memory_explore` | `{facet, path?, filter?, limit?, scan_limit?}` | `{facet, buckets: {value, count}[], total, missing, more_buckets, truncated}` |
| `memory_items_get` | `{ids, reach?}` | `{items: Hit[]}` |
| `memory_agents_list` | `{}` | `{root, agents: {agent_id, turns}[]}` |
| `memory_conversations_backfill_status` | `{}` | `{state: {phase, threads_total, threads_done, turns_stored, items_stored, error?, finished_at?}, pending_threads, pending_turns}` |
| `memory_conversations_backfill_start` | `{consent: true}` | same as status; runs in the background |
| `memory_brain_sources` | `{}` | `{root, sources: {source, documents}[], unfiled}` |
| `memory_brain_search` | `{query, source?, limit?}` | `{hits: Hit[]}` |
| `memory_brain_ingest` | `{path? \| text?, source?, title?}` | `{id, source, replayed}` |
| `memory_brain_forget` | `{source}` | `{forgotten}` |
| `memory_sources_list` | `{}` | `{sources: Source[]}` |
| `memory_sources_add` | `{kind, target, label?, schedule_mins?, namespace?}` | `{source: Source}` |
| `memory_sources_remove` | `{id, forget_items?}` | `{removed}` |
| `memory_sources_sync` | `{id?}` | `{started: string[]}` |
| `memory_jobs_list` | `{}` | `{pending: QueuedJob[], history: JobRun[]}` |
| `memory_jobs_run` | `{id?}` | `{runs: JobRun[]}`; ignores the build delay |
| `memory_import_scan` | `{}` | `{found, counts?: {documents, conversations, learnings}}` |
| `memory_import_start` | `{consent: true}` | `{state: ImportState}` |
| `memory_import_status` | `{}` | `{state: ImportState}` |
| `memory_import_retry_failed` | `{}` | `{state: ImportState}`; stores again the items a finished import skipped because the engine refused them (`ImportState.failed` counts them) |
| `memory_migration_scan` | `{}` | `{needed, shared}` |
| `memory_migration_start` | `{takeover?}` | `{state: MigrationState, running, interrupted}` |
| `memory_migration_status` | `{}` | `{state: MigrationState, running, interrupted}` |
| `memory_migration_retry` | `{}` | `MigrationState` |

`memory_context_*` and `memory_conversations_get/set` are retired.

**Moving into the per-user layout.** Memory written before layout v3 lives
under `app:tinymemory/…`. `memory_migration_*` stores each item again below
the person's `user:<id>` root, verifies it, switches the layout once every
item is copied, copies what arrived meanwhile, then removes the legacy copies
it verified. Progress is saved after every page in
`<workspace>/memory/layout_migration.json`, so the move resumes where it
stopped. The memory background job runs it on its own only while moving is
free (always off the hosted engine); `memory_migration_start` runs it now.
On a self-hosted CortexDB every account on the machine shares the legacy
tree: it moves only with `takeover: true`, and the first account to take it
claims it in `<app>/memory/legacy_claims/`; other accounts have nothing
to move and go on in their own per-user tree. A shared tree is cleaned up by
forgetting moved items by id, never by erasing a whole scope, since other
accounts may still write there. Items that could not be moved stay in the legacy tree;
`memory_migration_retry` puts them back in line.

**Past conversations.** `memory_conversations_backfill_start` walks the
thread store and stores every earlier turn of every thread the way the
lifecycle logs them (one item per message, the main agent's node, tagged
`backfill`). It stops at the first turn the lifecycle logged for a thread,
records progress in `<workspace>/memory/conversations_backfill.json`, and
needs `consent: true` because it uploads chat history.

**Forgetting a channel.** The session host records which channel each logged
thread arrived on (`<workspace>/memory/channel_threads.json`); disconnecting a
channel with `clear_memory` forgets those threads' conversations.

## UI

The Memory page lives under Connections at
`/connections?tab=brain&brain=<chip>`. Chips: `engine`, `ask` (recall, fetch
and the pack preview), `explorer`, `learnings` (built beliefs marked),
`conversations` (logging toggle, per-agent turns, backfill), `brain` (sources,
search, ingest, synced sources), `background` (the job queue) and `settings`
(the recall policy). Legacy `?brain=context` maps to `ask`, `documents`,
`sources`, `sync` and `history` to `brain`, `graph` and `goals` to `ask`.

Writes are visible as soon as they return (`WaitFor::Visible`); only beliefs
lag, built by a queued job at least `build_delay_secs` behind the writes. So
an empty `learnings` list while a `build_beliefs` job is pending says beliefs
are still being built, never that memory is empty.
