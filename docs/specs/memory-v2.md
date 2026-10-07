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
gate. A failing job is retried up to five times. The hosted engine builds on
its own schedule (`scheduled`); an engine that cannot consolidate answers
`skipped`.

## The brain (`memory::brain`, `memory::sources`)

- **Synced sources** (`folder`, `file`, `link`, `github`, `rss`, `composio`)
  are filed under the brain source their items belong to: GitHub → `github`,
  links and feeds → `web`, a Composio toolkit → its own source (`notion`,
  `gmail`, …), local files by type (`pdf`, HTML → `web`, other text →
  `markdown`). A source's `namespace` names the layout root it files under.
- **Ingest** (`memory_brain_ingest`) files a local file (converted, its
  source picked from its format) or text, accepted without waiting for
  indexing.
- Each ingest or sync queues one belief build per brain source it touched.

## Config (`config.toml`)

```toml
[memory]
engine = "tinyhumans"            # "tinyhumans" | "cortexdb"
# agent_id = "employee-7"        # host binding (see "Who is acting")
# root = "team:acme"

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
`UNSUPPORTED`, `INVALID_REQUEST`, `UNAUTHORIZED` or `ENGINE`.

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

`memory_context_*` and `memory_conversations_get/set` are retired.

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
