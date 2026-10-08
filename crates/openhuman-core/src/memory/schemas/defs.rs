//! [`ControllerSchema`] definitions for the `memory` namespace
//! (`openhuman.memory_*`, see `docs/specs/memory-v2.md`).

use crate::core::{ControllerSchema, FieldSchema, TypeSchema};

/// Every function of the namespace, in spec order.
pub const FUNCTIONS: [&str; 34] = [
    "engines_list",
    "engine_get",
    "engine_set",
    "policy_get",
    "policy_set",
    "pack_preview",
    "recall",
    "fetch",
    "learn",
    "forget",
    "items_list",
    "explore",
    "items_get",
    "agents_list",
    "conversations_backfill_status",
    "conversations_backfill_start",
    "brain_sources",
    "brain_search",
    "brain_ingest",
    "brain_forget",
    "sources_list",
    "sources_add",
    "sources_remove",
    "sources_sync",
    "jobs_list",
    "jobs_run",
    "import_scan",
    "import_start",
    "import_status",
    "import_retry_failed",
    "migration_scan",
    "migration_start",
    "migration_status",
    "migration_retry",
];

fn field(name: &'static str, ty: TypeSchema, comment: &'static str, required: bool) -> FieldSchema {
    FieldSchema {
        name,
        ty: if required {
            ty
        } else {
            TypeSchema::Option(Box::new(ty))
        },
        comment,
        required,
    }
}

fn opt(name: &'static str, ty: TypeSchema, comment: &'static str) -> FieldSchema {
    field(name, ty, comment, false)
}

fn req(name: &'static str, ty: TypeSchema, comment: &'static str) -> FieldSchema {
    field(name, ty, comment, true)
}

fn out(comment: &'static str) -> Vec<FieldSchema> {
    vec![req("result", TypeSchema::Json, comment)]
}

fn limit() -> FieldSchema {
    opt(
        "limit",
        TypeSchema::BoundedU64 { min: 1, max: 100 },
        "Most results (default 10).",
    )
}

fn filter() -> FieldSchema {
    opt(
        "filter",
        TypeSchema::Json,
        "MetaFilter: metadata fields, kinds, sources, tags_any, observed_after/before, and reach ({at: namespace, inherit, descendants}) to read only some memory nodes.",
    )
}

fn reach() -> FieldSchema {
    opt(
        "reach",
        TypeSchema::Json,
        "Reach {at: namespace, inherit, descendants}: only items in these memory nodes count; every node when omitted.",
    )
}

fn path() -> FieldSchema {
    opt(
        "path",
        TypeSchema::Json,
        "Explorer path: [{facet, value}], each step narrowing the items (see memory_explore).",
    )
}

fn cursor() -> FieldSchema {
    opt(
        "cursor",
        TypeSchema::String,
        "Engine cursor from a previous page.",
    )
}

/// The schema of `function`; an unknown function gets the namespace's
/// `unknown` placeholder, as every namespace does.
pub fn schema(function: &str) -> ControllerSchema {
    match function {
        "engines_list" => ControllerSchema {
            namespace: "memory",
            function: "engines_list",
            description: "List the memory engines this build offers and the active one.",
            inputs: vec![],
            outputs: out("{engines: EngineDescriptor[], active: string|null}"),
        },
        "engine_get" => ControllerSchema {
            namespace: "memory",
            function: "engine_get",
            description: "The configured memory engine, its credential and health.",
            inputs: vec![],
            outputs: out("{engine, endpoint?, has_key, status, reason?, fetch_modes}"),
        },
        "engine_set" => ControllerSchema {
            namespace: "memory",
            function: "engine_set",
            description: "Select a memory engine, optionally setting its endpoint and API key.",
            inputs: vec![
                    req("engine", TypeSchema::String, "Engine id: tinyhumans or cortexdb."),
                    opt("endpoint", TypeSchema::String, "Endpoint URL; empty clears it."),
                    opt("api_key", TypeSchema::String, "API key (cortexdb); empty removes it. Stored in the credential store, never in config."),
                ],
            outputs: out("Same as memory_engine_get."),
        },
        "recall" => ControllerSchema {
            namespace: "memory",
            function: "recall",
            description: "Ask memory a question; returns an answer with citations.",
            inputs: vec![req("question", TypeSchema::String, "The question."), filter(), limit()],
            outputs: out("{answer, citations: Citation[], model?}"),
        },
        "fetch" => ControllerSchema {
            namespace: "memory",
            function: "fetch",
            description: "Raw retrieval over stored items, filtered by metadata.",
            inputs: vec![
                    req("query", TypeSchema::String, "What to search for."),
                    opt("mode", TypeSchema::String, "keyword | vector | hybrid; limited to the engine's fetch_modes."),
                    filter(),
                    limit(),
                    cursor(),
                ],
            outputs: out("{hits: Hit[], next_cursor?}"),
        },
        "learn" => ControllerSchema {
            namespace: "memory",
            function: "learn",
            description: "Store a learning.",
            inputs: vec![
                    req("text", TypeSchema::String, "The learning."),
                    opt("kind", TypeSchema::String, "preference | fact | procedure | correction | other (default fact)."),
                    opt("confidence", TypeSchema::F64, "Confidence in 0..=1 (default 0.8)."),
                    opt("meta", TypeSchema::Json, "MemoryMeta to attach."),
                ],
            outputs: out("{id}"),
        },
        "forget" => ControllerSchema {
            namespace: "memory",
            function: "forget",
            description: "Remove items by id.",
            inputs: vec![req("ids", TypeSchema::Array(Box::new(TypeSchema::String)), "Item ids."), reach()],
            outputs: out("{forgotten: number}"),
        },
        "items_list" => ControllerSchema {
            namespace: "memory",
            function: "items_list",
            description: "Page through stored items, newest first.",
            inputs: vec![
                filter(),
                limit(),
                cursor(),
                path(),
                opt("preview", TypeSchema::Bool, "Snippet listing: a conversation or chunked document may carry only its start; read it whole with items_get."),
            ],
            outputs: out("{items: Hit[], next_cursor?}"),
        },
        "explore" => ControllerSchema {
            namespace: "memory",
            function: "explore",
            description: "Count stored items per value of one facet (kind, source, source_id, workspace, folder, file_path, language, repo, url, thread, agent, tool_call, tag), under an explorer path.",
            inputs: vec![
                req("facet", TypeSchema::String, "The facet to group by."),
                path(),
                filter(),
                opt("limit", TypeSchema::BoundedU64 { min: 1, max: 500 }, "Most buckets, largest first (default 50)."),
                opt("scan_limit", TypeSchema::BoundedU64 { min: 1, max: 50_000 }, "Most items a listing-based engine reads (default 5000)."),
            ],
            outputs: out("{facet, buckets: {value, count}[], total, missing, more_buckets, truncated}"),
        },
        "items_get" => ControllerSchema {
            namespace: "memory",
            function: "items_get",
            description: "Read stored items whole by id, in the order asked; unknown ids are left out.",
            inputs: vec![req("ids", TypeSchema::Array(Box::new(TypeSchema::String)), "Item ids (1 to 200)."), reach()],
            outputs: out("{items: Hit[]}"),
        },
        "policy_get" => ControllerSchema {
            namespace: "memory",
            function: "policy_get",
            description: "The memory lifecycle's policy: turn logging, the per-turn pack and its budgets, and the root and agent id work outside an agent resolves to.",
            inputs: vec![],
            outputs: out("{log_conversations, recall: {enabled, budget_tokens, learnings_limit, brain_limit, history_limit, team_limit, build_beliefs_every, pre_turn_timeout_ms, date_hint, compaction_timeout_ms, build_delay_secs}, root, agent_id, host_bound}"),
        },
        "policy_set" => ControllerSchema {
            namespace: "memory",
            function: "policy_set",
            description: "Change the memory lifecycle's policy.",
            inputs: vec![
                opt("log_conversations", TypeSchema::Bool, "Log every turn to the acting agent's conversations."),
                opt("recall_enabled", TypeSchema::Bool, "Give every turn a memory pack."),
                opt("budget_tokens", TypeSchema::BoundedU64 { min: 100, max: 16_000 }, "The pack's size in tokens."),
                opt("learnings_limit", TypeSchema::BoundedU64 { min: 0, max: 50 }, "Learnings and built beliefs per pack."),
                opt("brain_limit", TypeSchema::BoundedU64 { min: 0, max: 50 }, "Brain documents per pack."),
                opt("history_limit", TypeSchema::BoundedU64 { min: 0, max: 50 }, "This agent's earlier turns per pack."),
                opt("team_limit", TypeSchema::BoundedU64 { min: 0, max: 50 }, "Other agents' turns per pack; 0 leaves them out."),
                opt("build_beliefs_every", TypeSchema::BoundedU64 { min: 0, max: 1000 }, "Turns between belief builds; 0 turns them off."),
                opt("pre_turn_timeout_ms", TypeSchema::BoundedU64 { min: 100, max: 30_000 }, "How long a turn waits for its pack."),
            ],
            outputs: out("Same as memory_policy_get."),
        },
        "pack_preview" => ControllerSchema {
            namespace: "memory",
            function: "pack_preview",
            description: "The memory pack a turn would be given (with a query) or a session would start with (without one). Reads only.",
            inputs: vec![
                opt("query", TypeSchema::String, "What the turn says; omit to preview a session start."),
                opt("thread_id", TypeSchema::String, "Session start: the thread being resumed."),
                opt("agent_id", TypeSchema::String, "The memory agent to preview as; the default agent when omitted."),
            ],
            outputs: out("{agent_id, root, mode: turn|session, pack: {markdown, tokens, refs, sections, skipped, engine}}"),
        },
        "agents_list" => ControllerSchema {
            namespace: "memory",
            function: "agents_list",
            description: "The agents with logged conversations under the memory root, and how many turns each.",
            inputs: vec![],
            outputs: out("{root, agents: {agent_id, turns}[]}"),
        },
        "conversations_backfill_status" => ControllerSchema {
            namespace: "memory",
            function: "conversations_backfill_status",
            description: "Progress of storing past chats, and how many threads and turns from before turn logging are still unstored.",
            inputs: vec![],
            outputs: out("{state: {phase, threads_total, threads_done, turns_stored, items_stored, error?, finished_at?}, pending_threads, pending_turns}"),
        },
        "conversations_backfill_start" => ControllerSchema {
            namespace: "memory",
            function: "conversations_backfill_start",
            description: "Store past chats (turns from before turn logging) as the main agent's conversations, in the background. Uploads chat history to the selected engine, so it requires consent: true.",
            inputs: vec![req("consent", TypeSchema::Bool, "The user agreed to upload past chats to the engine.")],
            outputs: out("Same as memory_conversations_backfill_status."),
        },
        "sources_list" => ControllerSchema {
            namespace: "memory",
            function: "sources_list",
            description: "List document sources with their sync state.",
            inputs: vec![],
            outputs: out("{sources: Source[]}"),
        },
        "sources_add" => ControllerSchema {
            namespace: "memory",
            function: "sources_add",
            description: "Add a document source.",
            inputs: vec![
                    req("kind", TypeSchema::String, "folder | file | link | github | rss | composio."),
                    req("target", TypeSchema::String, "Path, URL, owner/repo, feed URL or Composio toolkit."),
                    opt("label", TypeSchema::String, "Display label (default: the target)."),
                    opt("schedule_mins", TypeSchema::BoundedU64 { min: 15, max: u64::from(u32::MAX) }, "Minutes between scheduled syncs; omit for on demand only."),
                    opt("namespace", TypeSchema::String, "Layout root to file the documents under, e.g. `team:acme`; the configured root (shared by every agent) when omitted."),
                ],
            outputs: out("{source: Source}"),
        },
        "sources_remove" => ControllerSchema {
            namespace: "memory",
            function: "sources_remove",
            description: "Remove a document source.",
            inputs: vec![
                    req("id", TypeSchema::String, "Source id."),
                    opt("forget_items", TypeSchema::Bool, "Also forget everything it stored."),
                ],
            outputs: out("{removed: boolean}"),
        },
        "sources_sync" => ControllerSchema {
            namespace: "memory",
            function: "sources_sync",
            description: "Start syncing one source, or all of them.",
            inputs: vec![opt("id", TypeSchema::String, "Source id; every source when omitted.")],
            outputs: out("{started: string[]}"),
        },
        "brain_sources" => ControllerSchema {
            namespace: "memory",
            function: "brain_sources",
            description: "The brain's sources (pdf, markdown, notion, github, web, …) and how many documents each holds.",
            inputs: vec![],
            outputs: out("{root, sources: {source, documents}[], unfiled}"),
        },
        "brain_search" => ControllerSchema {
            namespace: "memory",
            function: "brain_search",
            description: "Search the brain's documents, all sources or one.",
            inputs: vec![
                req("query", TypeSchema::String, "What to search for."),
                opt("source", TypeSchema::String, "One source's documents only."),
                limit(),
            ],
            outputs: out("{hits: Hit[]}"),
        },
        "brain_ingest" => ControllerSchema {
            namespace: "memory",
            function: "brain_ingest",
            description: "File a document in the brain: a local file (converted, its source picked from its format) or text. Queues the source's belief build.",
            inputs: vec![
                opt("path", TypeSchema::String, "A local file to read."),
                opt("text", TypeSchema::String, "Text to file."),
                opt("source", TypeSchema::String, "The source to file under (default: from the format, markdown for text)."),
                opt("title", TypeSchema::String, "A title."),
            ],
            outputs: out("{id, source, replayed}"),
        },
        "brain_forget" => ControllerSchema {
            namespace: "memory",
            function: "brain_forget",
            description: "Forget every document of one brain source.",
            inputs: vec![req("source", TypeSchema::String, "The source.")],
            outputs: out("{forgotten: number}"),
        },
        "jobs_list" => ControllerSchema {
            namespace: "memory",
            function: "jobs_list",
            description: "Memory's background job queue (belief builds, deferred ingests) and its latest runs.",
            inputs: vec![],
            outputs: out("{pending: QueuedJob[], history: JobRun[]}"),
        },
        "jobs_run" => ControllerSchema {
            namespace: "memory",
            function: "jobs_run",
            description: "Run queued background jobs now: one by id, or every pending one.",
            inputs: vec![opt("id", TypeSchema::String, "One job; every pending job when omitted.")],
            outputs: out("{runs: JobRun[]}"),
        },
        "import_scan" => ControllerSchema {
            namespace: "memory",
            function: "import_scan",
            description: "Look for a v1 memory store in this workspace.",
            inputs: vec![],
            outputs: out("{found, counts?: {documents, conversations, learnings}}"),
        },
        "import_start" => ControllerSchema {
            namespace: "memory",
            function: "import_start",
            description: "Import the v1 store into the selected engine. Uploads local data; requires consent: true.",
            inputs: vec![req("consent", TypeSchema::Bool, "Must be true.")],
            outputs: out("{state: ImportState}"),
        },
        "import_status" => ControllerSchema {
            namespace: "memory",
            function: "import_status",
            description: "Progress of the v1 import.",
            inputs: vec![],
            outputs: out("{state: ImportState}"),
        },
        "import_retry_failed" => ControllerSchema {
            namespace: "memory",
            function: "import_retry_failed",
            description: "Store again the items a finished v1 import skipped because the engine refused them.",
            inputs: vec![],
            outputs: out("{state: ImportState}"),
        },
        "migration_scan" => ControllerSchema {
            namespace: "memory",
            function: "migration_scan",
            description: "Whether memory from before the per-user layout is left to move.",
            inputs: vec![],
            outputs: out("{needed: bool, shared: bool}"),
        },
        "migration_start" => ControllerSchema {
            namespace: "memory",
            function: "migration_start",
            description: "Move memory from before the per-user layout now. takeover: true agrees to take a legacy tree other accounts on this machine may share.",
            inputs: vec![opt("takeover", TypeSchema::Bool, "Consent to take a shared legacy tree.")],
            outputs: out("{state: MigrationState, running: bool, interrupted: bool}"),
        },
        "migration_status" => ControllerSchema {
            namespace: "memory",
            function: "migration_status",
            description: "Progress of the move into the per-user layout.",
            inputs: vec![],
            outputs: out("{state: MigrationState, running: bool, interrupted: bool}"),
        },
        "migration_retry" => ControllerSchema {
            namespace: "memory",
            function: "migration_retry",
            description: "Put the items the move could not store back in line for the next run.",
            inputs: vec![],
            outputs: out("MigrationState"),
        },
        _ => ControllerSchema {
            namespace: "memory",
            function: "unknown",
            description: "Unknown memory controller function.",
            inputs: vec![],
            outputs: vec![req("error", TypeSchema::String, "Lookup error details.")],
        },
    }
}
