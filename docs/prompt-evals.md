# Prompt evals

Two tiers answer two different questions. Do not cite one as evidence for the
other.

| | Tier 1 — routing | Tier 2 — comprehension |
|---|---|---|
| Question | Is the agent wired so a model *could* follow its prompt? | Does a real model actually follow it? |
| Where | `tests/agent_prompt_comprehension_e2e.rs` | `scripts/prompt-eval.sh` + `scripts/prompt-eval/cases.json` |
| Model | Scripted completions (no model) | The real backend model |
| Cost | Free, deterministic | Money per run, non-deterministic |
| CI | Runs with the Rust integration tests | **Never.** Refuses to run when `CI=true` |

## Tier 1 pins the script, not a model's judgment

Every tier-1 completion is scripted. When the `workflow_builder` case scripts
two `search_tool_catalog` calls and then `propose_workflow`, the test proves
the builder's belt carries both tools, that both calls resolve (no
`unknown tool`), that `flows_build` extracts the proposal, and that nothing in
the runtime turns two searches into three. It proves **nothing** about whether
a model, given that prompt, would propose instead of searching 27 times. That
was the 2026-09-17 incident: the right guidance was in the prompt, and the
model never reached it.

A green tier 1 is therefore not evidence of comprehension. Only tier 2 is.

The six cases cover the agents where mis-routing has cost something:
`workflow_builder` (must reach `propose_workflow`, never three consecutive
catalog searches), `orchestrator` (hands integration work off, never holds the
raw Composio or cron tools), `integrations_agent`, `scheduler_agent`, and the
two zero-belt agents `summarizer` and `trigger_triage` (advertise nothing).
Fleet-wide static coverage of all agents lives in the prompt tests under
`crates/openhuman-core/src/agent/registry/agents/`, not here.

```sh
cargo test -p openhuman --test agent_prompt_comprehension_e2e
```

## Tier 2

```sh
cargo build --bin openhuman-core
OPENHUMAN_BACKEND_SESSION_TOKEN=... scripts/prompt-eval.sh --case workflow-builder-news
# or OPENHUMAN_BACKEND_API_KEY=...; BACKEND_URL selects the backend
```

Each case gets a fresh `mktemp -d` workspace (`OPENHUMAN_WORKSPACE`) and its own
`openhuman-core call` subprocesses: one to install the credential, one to run
the agent (`openhuman.flows_build` or `openhuman.agent_chat`), and one for the
judge. A process per case sidesteps the process-global model override and
`AlreadyRunning`, and needs no feature gate.

Scoring reads only artifacts the run already writes, in this order:

1. **Hard failure signals.** A breaker halt (the repeat/failure middlewares'
   halt log line), `[SUBAGENT_INCOMPLETE]` in a transcript, or `trail_off` /
   `capped` / `error` on the `flows_build` result. Any of these scores the run
   0: it was unproductive. The motivating incident would have tripped these.
2. **Did it do the thing.** Expected calls, forbidden calls and repeat caps,
   from the assistant `tool_calls` in `<ws>/**/session_raw/*.jsonl`.
3. **Cost.** Summed from each transcript's `_meta` (`input_tokens`,
   `output_tokens`, `cached_input_tokens`, `charged_amount_usd`), plus an
   `max_input_tokens` ceiling per case. The judge call is not included.
4. **Judge** (cases with `"judge": true`). The production close-verification
   rubric from `close_verification_prompt` in
   `agent/harness/session/turn_checkpoint.rs`, copied into `cases.json` rather
   than widening that `pub(super)` function for an on-demand script. The
   verdict is read as `parse_close_verdict` reads it: the last standalone
   `ACCEPT`/`REJECT` token wins, so `UNACCEPTABLE` is not `ACCEPT`.

One row per case is appended to `target/prompt-eval-runs.jsonl`
(gitignored with `target/`), and the run prints the total USD.

**Every row records the model id beside its cost.** A provider-side model
update silently rebaselines every score. Compare rows only when the model
ids match.

Tier 2 reads the on-disk transcripts, never Langfuse: Langfuse push is
disabled outside staging/dev by design, and only the web progress bridge
installs a collector at all. The run journal under
`<ws>/tinyagents_store/journal` is the natural enrichment if transcript
scoring proves too coarse.

### Adding a case

Add an object to `cases` in `scripts/prompt-eval/cases.json`: `id`, `entry`
(`flows_build` or `agent_chat`), `message`, `expect_calls`, `forbid_calls`,
`max_consecutive` (`{tool: cap}`), `max_input_tokens`, `judge`, and a `_why`
naming the failure it guards against. Keep the set small; every case costs
money on every run.
