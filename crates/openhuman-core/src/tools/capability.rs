//! Memory-capability classification of agent tools.
//!
//! Split out of `tools/ops.rs`, which registers the tools this classifies, so
//! the registration file stays inside its layout limit.

/// Classify an agent tool into the memory capability family its surface
/// requires, so [`all_tools_with_runtime`](super::ops::all_tools_with_runtime) can drop tools the bound memory
/// driver does not advertise (`docs/specs/kernel.md` §3.3).
///
/// `None` means "not backed by the memory driver" — a workspace file
/// (`update_memory_md`), the per-workspace people SQLite store, pure
/// introspection (`memory_store_kinds`), or a flow-sandboxed namespace
/// (`flow_memory_*`, already `DomainGroup::Flows`). Such a tool is never
/// filtered on the capability axis. `None` here is a *decision*, not a default:
/// `every_memory_tool_has_an_explicit_capability_or_is_core` forces every
/// memory-family tool through this function so a new one cannot land in the
/// always-present bucket by accident.
///
/// The mandatory families ([`Capability::Core`], [`Capability::Recall`]) are
/// returned explicitly rather than folded into `None`. Against a *driver's*
/// advertised set the filter is a no-op for them by construction (a bindable
/// driver always advertises `Capability::MANDATORY`) — but it is load-bearing
/// for one host decision below the driver: `CoreContext::memory_capabilities`
/// answers with the empty set for a deliberate `[subsystems.memory] driver =
/// "null"`, and that is what drops `memory_store` / `memory_forget` / the
/// recall tools when an operator turns memory off. Folding them into `None`
/// would leave an agent able to persist, expose or delete memory through the
/// session builder's own `Arc<dyn Memory>` in exactly that configuration.
///
/// **The `memory_` prefix is deliberately NOT a catch-all here.** `tools::ops::tool_group`
/// can prefix-match because every `memory_*` tool is one family on the
/// *DomainSet* axis; on the capability axis the family differs per tool, and a
/// wrong default is worse than no rule. Hence enumeration plus two narrow
/// prefix rules, backed by the drift guard.
///
/// ## Honesty clause — two assignments still run ahead of the plumbing:
/// `tool_stats` reads the legacy `Arc<dyn Memory>` + `tool_tracker`, not
/// `MemoryToolMemory`; `memory_diff` reads `memory::diff::ops`, not
/// `MemoryDiff`. Filtering both on the driver's advertised set is still the
/// correct M5 behaviour: §3.3 contracts what the *model is told exists*, so
/// the later re-point onto `MemoryGuard` must not change the advertised
/// surface, and `None` to dodge the mismatch would bake the wrong contract in.
/// `goals_*` was the third until #5560 routed it onto the guarded
/// `MemoryGoals` family — the advertised capability did not change when
/// the plumbing caught up: the exact property this clause protects.
pub(crate) fn tool_capability(name: &str) -> Option<tinymemory_api::capabilities::Capability> {
    use tinymemory_api::capabilities::Capability;

    // Not driver-backed. Each entry is an argued exception, not a fallthrough.
    if name == "update_memory_md"          // writes the workspace `MEMORY.md` file directly
        || name == "memory_store_kinds"    // enumerates `MemoryKind` constants; no store access
        || name.starts_with("flow_memory_")
    // flow-sandboxed; DomainGroup::Flows
    {
        return None;
    }

    let capability = match name {
        // ── Mandatory families: always advertised, listed for the record ──
        // The collapsed `memory` tool is `Core` because `store` and `forget`
        // are: it must stay registered whenever the mandatory family is, and
        // it filters its own action list by capability so an unavailable
        // action is never advertised. See `memory::tools::collapsed`.
        "memory" | "memory_store" | "memory_forget" | "remember_preference" | "save_preference" => {
            Capability::Core
        }
        // Chunk/recall retrieval surface. NOT `Tree` — these read chunk
        // embeddings and chunk rows, never the summary tree.
        "memory_recall"
        | "memory_vector_search"
        | "memory_chunk_context"
        | "memory_hybrid_search"
        | "memory_store_raw_chunks" => Capability::Recall,

        // ── Optional families: absence means the tool disappears ──
        // The one registered tree tool (`MemoryQueryTool` is an alias of
        // `MemoryTreeTool`, `memory/query/mod.rs`) plus the compiled persona
        // flavour reader, which reads a flavoured summary-tree root.
        "memory_tree" | "memory_flavour" => Capability::Tree,
        // Free-text search over the canonical *entity* index
        // (`memory::tree::retrieval::search::search_entities`).
        "memory_store_raw_search" => Capability::Entities,
        "memory_diff" => Capability::Diff,
        "memory_doctor" => Capability::Maintenance,
        "tool_stats" => Capability::ToolMemory,

        // The long-term goals tool. It was four `goals_*` tools and is now one
        // `op`-dispatched `goals`; the exact arm is what the prefix rule below
        // no longer covers. The per-thread `goal_get`/`goal_set`/
        // `goal_complete` tools are `DomainGroup::Threads` and a different
        // concept, and neither `goals` nor `goals_` catches them.
        "goals" => Capability::Goals,

        // Prefix rules, so a NEW tool in one of these families auto-gates
        // instead of silently landing in the un-filtered bucket — the same
        // reasoning as `tool_group`'s prefix families (#4808 review). Ordered
        // after the exact arms so `memory_tree` is not swallowed by
        // `memory_tree_`. The underscore in `goals_` is load-bearing: the
        // per-thread `goal_get`/`goal_set`/`goal_complete` tools are
        // `DomainGroup::Threads` and must not be caught.
        n if n.starts_with("goals_") => Capability::Goals,
        n if n.starts_with("memory_tree_") => Capability::Tree,

        _ => return None,
    };
    Some(capability)
}
