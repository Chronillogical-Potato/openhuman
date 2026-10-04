//! [`ControllerSchema`] definitions for the `memory` namespace
//! (`openhuman.memory_*`, see `docs/specs/memory-v2.md`).

use crate::core::{ControllerSchema, FieldSchema, TypeSchema};

/// Every function of the namespace, in spec order.
pub const FUNCTIONS: [&str; 24] = [
    "engines_list",
    "engine_get",
    "engine_set",
    "recall",
    "fetch",
    "learn",
    "forget",
    "items_list",
    "explore",
    "items_get",
    "conversations_get",
    "conversations_set",
    "conversations_backfill_status",
    "conversations_backfill_start",
    "sources_list",
    "sources_add",
    "sources_remove",
    "sources_sync",
    "context_get",
    "context_refresh",
    "context_set",
    "import_scan",
    "import_start",
    "import_status",
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

fn namespace() -> FieldSchema {
    opt(
        "namespace",
        TypeSchema::String,
        "Memory node, e.g. `agent:researcher` or `team:acme/agent:writer`; the root when omitted.",
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
            inputs: vec![filter(), limit(), cursor(), path()],
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
        "conversations_get" => ControllerSchema {
            namespace: "memory",
            function: "conversations_get",
            description: "Conversation ingestion settings and the latest stored batches.",
            inputs: vec![],
            outputs: out("{enabled, batch_turns, idle_secs, recent}"),
        },
        "conversations_set" => ControllerSchema {
            namespace: "memory",
            function: "conversations_set",
            description: "Change conversation ingestion settings.",
            inputs: vec![
                    opt("enabled", TypeSchema::Bool, "Store conversations."),
                    opt("batch_turns", TypeSchema::BoundedU64 { min: 1, max: 100 }, "Turns per stored batch."),
                    opt("idle_secs", TypeSchema::BoundedU64 { min: 1, max: 86_400 }, "Idle seconds before a partial batch is stored."),
                ],
            outputs: out("Same as memory_conversations_get."),
        },
        "conversations_backfill_status" => ControllerSchema {
            namespace: "memory",
            function: "conversations_backfill_status",
            description: "Progress of storing past chats, and how many threads and turns from before automatic ingestion are still unstored.",
            inputs: vec![],
            outputs: out("{state: {phase, threads_total, threads_done, turns_stored, items_stored, error?, finished_at?}, pending_threads, pending_turns}"),
        },
        "conversations_backfill_start" => ControllerSchema {
            namespace: "memory",
            function: "conversations_backfill_start",
            description: "Store past chats (turns from before automatic ingestion) as conversation memory, in the background. Uploads chat history to the selected engine, so it requires consent: true.",
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
                    opt("namespace", TypeSchema::String, "Memory node to store the documents at, e.g. `agent:researcher`; the root (shared by every agent) when omitted."),
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
        "context_get" => ControllerSchema {
            namespace: "memory",
            function: "context_get",
            description: "The compiled context.md of one memory node (the root by default) and its settings.",
            inputs: vec![namespace()],
            outputs: out("{namespace, markdown, tokens, generated_at, interval_mins, budget_tokens, enabled}"),
        },
        "context_refresh" => ControllerSchema {
            namespace: "memory",
            function: "context_refresh",
            description: "Recompile one memory node's context.md now (the root by default).",
            inputs: vec![namespace()],
            outputs: out("Same as memory_context_get."),
        },
        "context_set" => ControllerSchema {
            namespace: "memory",
            function: "context_set",
            description: "Change context.md settings.",
            inputs: vec![
                    opt("enabled", TypeSchema::Bool, "Compile and inject context.md."),
                    opt("interval_mins", TypeSchema::BoundedU64 { min: 5, max: u64::from(u32::MAX) }, "Minutes between recompiles."),
                    opt("budget_tokens", TypeSchema::BoundedU64 { min: 100, max: 32_000 }, "Token budget."),
                ],
            outputs: out("Same as memory_context_get."),
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
        _ => ControllerSchema {
            namespace: "memory",
            function: "unknown",
            description: "Unknown memory controller function.",
            inputs: vec![],
            outputs: vec![req("error", TypeSchema::String, "Lookup error details.")],
        },
    }
}
