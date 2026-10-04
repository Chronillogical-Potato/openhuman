# Memory v2 in OpenHuman

Status: accepted. It replaces the whole v1 memory surface (the `memory_tree`,
`memory_goals`, `people`, `tree_summarizer`, `slack_memory` and `memory_sync`
namespaces, and the old `memory.*` and `memory_sources.*` methods). The engine
contract lives in TinyMemory: `vendor/tinymemory/docs/specs/memory-v2.md`.

## Model

| Concept | Meaning |
| --- | --- |
| Engine | Who stores and answers. At launch: `tinyhumans` (hosted CortexDB, needs sign-in) and `cortexdb` (your own CortexDB, endpoint + key). |
| Recall | Ask a question, get an answer with citations (engine-implemented). |
| Fetch | Raw keyword/vector/hybrid search with metadata filters. |
| Store | Documents (synced sources), Conversations (auto, after turns), Learnings (explicit). |
| context.md | Periodically compiled brief, one per memory node, injected as the first user message of a new session. |
| Namespace | The memory node an item lives at. The root is shared by every agent; every other agent has its own node. |

With no usable engine (signed out and no CortexDB key), memory is **off**:
- the tools are not registered;
- ingestion is a no-op;
- RPCs answer `MEMORY_OFF`;
- the UI explains why.

## Config (`config.toml`)

```toml
[memory]
engine = "tinyhumans"            # "tinyhumans" | "cortexdb"

[memory.engines.cortexdb]
endpoint = "https://api-v1.cortexdb.ai"   # key in keychain as "memory-cortexdb"

[memory.conversations]
enabled = true
batch_turns = 4      # store after this many committed turns in a thread…
idle_secs = 120      # …or after the thread is idle this long

[memory.context]
enabled = true
interval_mins = 360
budget_tokens = 2000

[memory]
root_agents = ["orchestrator"]   # agents that read and write the shared root node

[memory.agents.researcher]       # optional, per agent
namespace = "project:q4"         # pin the agent to a node (default agent:<id>)
inherit = true                   # also read the nodes above (default true)
context = true                   # its own context.md (default [memory.context] enabled)
```

A `[[memory.sources]]` entry may set `namespace` to store its documents at a
node other than the root.

Old `[subsystems.memory]` and v1 `[memory]` keys are ignored.

Out of scope: `memory::conversations` (the chat thread/message JSONL store over
`tinymemory-conversations`) is thread persistence, not memory. The store moves
to TinyAgents as `tinyagents_session::threads`, keeping the same on-disk format.
The host's thread code moves from `memory::conversations` to
`threads::store`, which wraps it, so that `memory/` holds only v2.

## Agent namespaces

Memory is a tree of nodes (`tinymemory::Namespace`; see the TinyMemory spec's
*Namespaces*). On CortexDB each node keeps learnings, documents and
conversations as separate scopes:
`app:tinymemory/<node segments>/app:{learnings,documents,conversations}`; the
root node keeps the original `app:tinymemory/app:*` scopes, so everything
stored before namespaces is root memory.

- **Who acts.** `memory::scope` scopes a `MemoryIdentity` (the agent, the
  agents that spawned it, its team) around every turn: the session host and
  channel dispatch for top-level agents, the sub-agent runner for children
  (nested automatically), the team runtime for members. It needs no config;
  memory resolves it against its own.
- **Which node.** `root_agents` (the main chat agent by default) use the root
  (or their team's node); any other agent `agent:<id>`, nested under the
  agent that spawned it (`agent:researcher/agent:scout`); a team member
  `team:<team>/agent:<id>`; `[memory.agents.<id>] namespace` pins one.
- **What it reads.** Its node and, unless `inherit = false`, the nodes above
  it, never a sibling's. The `memory` tool overwrites any `reach` in the
  model's filter and confines `forget` to the reach.
- **What it writes.** `learn` stores at the agent's node, or with
  `share: true` at the nearest shared node above it (its team's, else the
  root). Conversations are stored at the answering agent's node; a batch never
  mixes two agents. Backfill and import write the root.

## Agent tool: `memory`

There is one tool. Its `action` is `recall` | `fetch` | `learn` | `forget`:
- `recall { question, filter? }` returns `{answer, citations[]}`.
- `fetch { query, mode?, filter?, limit? }` returns `{hits[]}`. `mode` is limited to the engine's `fetch_modes`.
- `learn { text, kind?, confidence?, share? }` returns `{id}`. The host fills `meta` with:
  - `namespace` (the agent's node, or its shared node with `share: true`);
  - `workspace` (the agent's `action_dir`);
  - `thread_id` and `agent_id`;
  - `tool_call` (this call's name and id);
  - `source.kind = "agent"`.
- `forget { ids }` returns `{forgotten}`, counting only items in the agent's reach.

`recall` and `fetch` read only the agent's reach.

## Automatic ingestion

- **Conversations:** a bus subscriber on turn commit buffers per thread. It stores one `Conversation` item when `batch_turns` is reached or when the thread has been idle for `idle_secs`. Meta carries `thread_id`, `agent_id`, `workspace`, `turns` and `tool_calls` (name and id only; arguments are never stored).
- **Documents:** a registry of sources (`folder`, `file`, `link`, `github`, `rss`, `composio`). Sync runs on demand and on a scheduler job. Each item gets `folder`, `file_path`, `language`, `repo`, `commit` and `url` where they apply.

## context.md

- Every memory node can have its own: the root's is `<workspace>/memory/context.md`, any other node's `<workspace>/memory/context/<kind>-<id>/…/context.md` (`context/agent-researcher/context.md`). Each is compiled from that node's reach.
- The cron job `memory_context_refresh` runs every `interval_mins` and refreshes the root, every node with a document and every `[memory.agents.<id>]` agent. `memory_context_refresh { namespace? }` refreshes one node on demand.
- On a **new** session the session host prepends the acting agent's document, wrapped in `<memory-context>…</memory-context>`, as the first user message, next to the workflows context. A node with no document yet gets its nearest ancestor's and has its own compiled in the background.
- Resumed sessions keep their frozen transcript.

## RPC (`openhuman.memory_*`)

All methods take and return JSON objects. Errors use the standard structured error; the `code` is one of `MEMORY_OFF`, `UNSUPPORTED`, `INVALID_REQUEST`, `UNAUTHORIZED` or `ENGINE`.

| Method | Params | Result |
| --- | --- | --- |
| `memory_engines_list` | `{}` | `{engines: EngineDescriptor[], active: string\|null}` |
| `memory_engine_get` | `{}` | `{engine: string\|null, endpoint?: string, has_key: boolean, status: "ok"\|"degraded"\|"down"\|"off", reason?: string, fetch_modes: string[]}` |
| `memory_engine_set` | `{engine, endpoint?, api_key?}` | same as `engine_get` |
| `memory_recall` | `{question, filter?, limit?}` | `{answer, citations: Citation[], model?}` |
| `memory_fetch` | `{query, mode?, filter?, limit?, cursor?}` | `{hits: Hit[], next_cursor?}` |
| `memory_learn` | `{text, kind?, confidence?, meta?}` | `{id}` |
| `memory_forget` | `{ids, reach?}` | `{forgotten: number}`; with `reach`, only items in it |
| `memory_items_list` | `{filter?, limit?, cursor?, path?}` | `{items: Hit[], next_cursor?}` |
| `memory_conversations_backfill_status` | `{}` | `{state: {phase, threads_total, threads_done, turns_stored, items_stored, error?, finished_at?}, pending_threads, pending_turns}` |
| `memory_conversations_backfill_start` | `{consent: true}` | same as status; runs in the background |
| `memory_explore` | `{facet, path?, filter?, limit? (1–500, default 50), scan_limit?}` | `{facet, buckets: {value, count}[], total, missing, more_buckets, truncated}` |
| `memory_items_get` | `{ids, reach?}` (1–200) | `{items: Hit[]}` in the order asked; unknown ids (and ids beyond `reach`) left out |
| `memory_conversations_get` | `{}` | `{enabled, batch_turns, idle_secs, recent: {thread_id, turns, stored_at}[]}` |
| `memory_conversations_set` | `{enabled?, batch_turns?, idle_secs?}` | same as `conversations_get` |
| `memory_sources_list` | `{}` | `{sources: Source[]}` |
| `memory_sources_add` | `{kind, target, label?, schedule_mins?, namespace?}` | `{source: Source}` |
| `memory_sources_remove` | `{id, forget_items?}` | `{removed: boolean}` |
| `memory_sources_sync` | `{id?}` (all if omitted) | `{started: string[]}` |
| `memory_context_get` | `{namespace?}` (root when omitted) | `{namespace, markdown, tokens, generated_at\|null, interval_mins, budget_tokens, enabled}` |
| `memory_context_refresh` | `{namespace?}` | same as `context_get` |
| `memory_context_set` | `{enabled?, interval_mins?, budget_tokens?}` | same as `context_get` |
| `memory_import_scan` | `{}` | `{found: boolean, counts?: {documents, conversations, learnings}}` |
| `memory_import_start` | `{consent: true}` | `{state: ImportState}` |
| `memory_import_status` | `{}` | `{state: ImportState}` |

The types:
- `EngineDescriptor`:
  - Identity: `id`, `label`, `description`.
  - Requirements: `hosted`, `needs_endpoint`, `needs_key`, `default_endpoint`.
  - `fetch_modes` (a subset of `"keyword"`, `"vector"` and `"hybrid"`).
- `Hit`: `{id, kind: "document"|"conversation"|"learning", text, meta: MemoryMeta, score}`.
- `Citation`: `Hit` with `snippet` in place of `text`.
- `MemoryMeta` uses the TinyMemory field names in snake_case. `source` is `{kind, id?}`.
- `MetaFilter` uses the same fields, plus `kinds`, `sources`, `tags_any`, `observed_after` and `observed_before`.
- `Source`:
  - Identity: `id`, `kind`, `target` (path, URL, `owner/repo`, feed URL or Composio toolkit), `label`.
  - Sync state: `schedule_mins`, `last_sync_at`, `status` (`"idle"`, `"syncing"` or `"error"`), `error?`, `items`.
- `ImportState`: `{phase: "idle"|"running"|"done"|"error", imported, total, error?}`.

`memory_import_start` without `consent: true` is refused. It uploads local data to the selected engine.

## UI

The Memory page lives under Connections at `/connections?tab=brain&brain=<chip>`. Its chips are `engine`, `ask`, `explorer`, `learnings`, `conversations`, `documents` and `context`. The default chip is `ask` when an engine is active and `engine` otherwise.

**Past conversations.** Live ingestion only sees turns committed while it is on. `memory_conversations_backfill_start` walks the thread store and stores every earlier turn in the same `batch_turns` items and metadata live ingestion uses, tagged `backfill`. Each user message opens a turn and the replies after it are its answer. It stops short of the turns live ingestion has counted for a thread (its most recent ones), records how far each thread was stored in `<workspace>/memory/conversations_backfill.json` (resumable, and a later run sends only what is new), and needs `consent: true` because it uploads chat history. The legacy import and the backfill both write through `MemoryEngine::store_many` in batches of 25.

**Explorer.** `memory_explore` groups stored items by one of TinyMemory's standard facets (`kind`, `namespace`, `source`, `source_id`, `workspace`, `folder`, `file_path`, `language`, `repo`, `url`, `thread`, `agent`, `tool_call`, `tag`) and counts each value. The explorer path is a list of `{facet, value}` steps that the core turns into a filter with `Facet::narrow`, so `memory_explore` and `memory_items_list` take the same `path` and the UI never rebuilds filters itself. Opening an item reads it whole with `memory_items_get`. Facets belong to the TinyMemory contract (`MemoryEngine::explore`/`get`, with listing-based defaults), not to an engine's storage layout.

Legacy `?brain=graph|goals|sync|sources` values map to `ask`, `ask`, `documents` and `documents`. `/settings/memory-engine` redirects to the `engine` chip.
