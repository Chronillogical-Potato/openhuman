# bin

Auxiliary binaries declared as `[[bin]]` targets in
`crates/openhuman-cli/Cargo.toml`, next to the primary `openhuman-core`
binary (`src/main.rs`, described in the [crate README](../../README.md)). One
is a test fixture, one is an experimental multi-tenant supervisor, and the
rest are benchmarks. None of them ship in the desktop product.

## How it works

Each binary is a separate `[[bin]]` entry (`autobins = false`), and the
heavier ones carry `required-features` so a plain build skips them instead of
failing to link:

| Binary | Source | Required features | Purpose |
| --- | --- | --- | --- |
| `test-mcp-stub` | `test_mcp_stub.rs` | none | Minimal stdio MCP server that tests spawn. |
| `openhuman-fleet` | `fleet.rs` | `http-server`, `bin-tools` | Process-per-user supervisor and reverse proxy. |
| `tool-search-bench` | `tool_search_bench.rs` | none (`jev`, in `default`, for the Jev ranker) | Accuracy and cost of each `tool_search` ranker. |
| `tool-dialect-bench` | `tool_dialect_bench.rs` | none | Manual A/B of text tool-call dialects against a local Ollama model. |
| `rss-bench` | `rss_bench.rs` | `rss-bench` | Steady-state RSS of an embedded agent roster. |
| `library-profile` | `library_profile/main.rs` | `rss-bench` (add `rss-bench-dhat` for heap profiles) | Hermetic library profiling scenarios. |

`http-server` and `jev` are in `default`; `bin-tools`, `rss-bench` and
`rss-bench-dhat` are not. A plain `cargo build -p openhuman-cli` therefore
produces `openhuman-core`, `test-mcp-stub`, `tool-search-bench` and
`tool-dialect-bench`.

### test-mcp-stub

Speaks just enough MCP to answer `initialize`, `tools/list` and `tools/call`
for one `echo` tool, over newline-delimited JSON-RPC on stdin and stdout, and
exits when stdin closes. `initialize` reports `PROTOCOL_VERSION`
(`2025-11-25`). It depends on nothing beyond `serde_json`. Tests spawn it
through `env!("CARGO_BIN_EXE_test-mcp-stub")`, which makes Cargo build it for
every test run: `tests/mcp_registry_e2e.rs`,
`tests/mcp_registry_multi_server.rs`, `tests/agent_harness_e2e.rs`,
`tests/json_rpc_e2e.rs`, `tests/in_process/domain_modules_e2e.rs` and
`tests/raw_coverage/tool_registry_approval_raw_coverage_e2e.rs`.

### openhuman-fleet

Hosts one `openhuman-core` process per user or workspace behind a single
endpoint, so a team server can run many members' assistants while every
existing client (`CloudHttpTransport`) keeps working unchanged. The design is
process-per-user, not in-process multi-tenancy:

```text
 client --POST /{user_id}/rpc + edge token--> openhuman-fleet (--listen)
                                                 |  checks EdgeToken
                                                 |  swaps in CoreBearer
                                                 v
                    openhuman-core run --headless-api --port base+N
                    OPENHUMAN_WORKSPACE=<workspaces_root>/<user>
                    OPENHUMAN_CORE_TOKEN=<core bearer>
```

- Each tenant is its own OS process with its own workspace volume and its own
  core bearer. Tenants do not yet run under distinct OS users or containers,
  so this MVP is not a production multi-tenant security boundary for
  arbitrary agent tools.
- The supervisor mints a distinct edge token per tenant for clients and is
  the only holder of the tenants' core bearers. `EdgeToken` and `CoreBearer`
  are separate newtypes so they cannot be confused. Minted edge tokens are
  written to the file named by `--edge-token-output`.
- The proxy forwards `POST /{user_id}/rpc` verbatim to
  `http://127.0.0.1:<port>/rpc`, so the JSON-RPC wire contract is unchanged
  end to end.

Flags: `--listen` (default `127.0.0.1:8899`), `--workspaces-root` (default
`./fleet-workspaces`), `--core-bin` (default `openhuman-core`),
`--base-core-port` (default 7900; tenant N listens on base + N), `--users`
(comma-separated ids to provision at boot) and `--edge-token-output`
(required). Ports are assigned sequentially, and each tenant must pass an
authenticated JSON-RPC readiness probe before it is registered. A production
supervisor would read each core's bound port from a ready file
(`EmbeddedReadySignal`) and reconcile membership against
`tinyhumansai/backend`.

```bash
cargo build -p openhuman-cli --features bin-tools --bin openhuman-fleet
```

### tool-search-bench

Measures how well each `tool_search` ranker picks the right tool for an
intent. The catalogue is real: every tool the orchestrator session registers
(built as a session builds it, in a temp workspace), plus the recorded
Composio catalogues under `tests/fixtures/composio_*.json` (about 1,000
actions across nine toolkits) as deferred per-action tools. The intents are
`tests/fixtures/tool_search/intents.jsonl`, each labelled with the tool it
should reach, or `none`.

Rankers: `bm25` (`tinytools::Bm25Ranker`, the harness fallback), `overlap`
(`tinyagents_harness::tool::select::rank_tools_by_prompt`), `embedding`
(only with `--embedding`) and `jev` (`TinyHumansJevRanker` or a direct
`JevRanker`). `jev` needs a key: `OPENHUMAN_BACKEND_API_KEY` through the
TinyHumans proxy (`BACKEND_URL` overrides the base) or `TYPESAFE_API_KEY`;
it is skipped without one. The report gives top-1 and top-3 accuracy, the
retriever's recall (the ceiling Jev can reach), the needless rate (a `none`
row answered with a hit), p50 and p95 latency, input tokens, USD from the
provider's `usage`, and a per-family confusion table.

Flags: `--ranker <all|bm25|overlap|embedding|jev>`, `--intents <path>`,
`--top-k`, `--retrieval-k`, `--family`, `--embedding`, `--misses`,
`--json <path>`, `--dump-catalogue`.

```bash
cargo run -p openhuman-cli --bin tool-search-bench -- --ranker all
cargo run -p openhuman-cli --bin tool-search-bench -- --dump-catalogue
```

### tool-dialect-bench

A manual, network-touching A/B of `agent.tool_dispatcher` values (`xml`,
`pformat`, `python`, `typescript`) against a local Ollama model. For every
dialect and task it composes the system prompt the way `ToolsSection` and the
dialect's protocol block do, sends one user turn with no schemas on the wire,
parses the answer with `tinytools_agent::parse_text` and the harness
registry, and records provider-reported prompt and output tokens,
system-prompt bytes, call recovery, tool-name and argument accuracy, latency
and the `CallSource`. It prints a markdown summary table; `--json` appends
every row. CI never runs it.

```bash
ollama pull qwen3:8b
cargo run -p openhuman-cli --bin tool-dialect-bench -- \
    --model qwen3:8b --dialects xml,pformat,python,typescript --trials 3
```

`OLLAMA_HOST` or `OPENHUMAN_LOCAL_INFERENCE_URL` picks the server, as for the
product. `--tasks 0,5` narrows to fixture tasks, `--max-output-tokens N`
raises the per-call cap for thinking models, and `-v` prints each prompt and
answer.

### rss-bench

Steady-state RSS benchmark for an embedded agent roster (#5046). It mirrors
the OpenCompany embedding contract: a bare `OpenHumanSessionHost` built
through `OpenHumanSessionHost::builder` (no `CoreBuilder`, no RPC, no
background services) with an injected mock model and a per-agent temp
workspace. It builds a 1-agent and an 8-agent roster, runs one deterministic
warm-up turn per agent, settles, then samples
`/proc/self/{status,smaps_rollup}`.

There are two modes. `--child --roster N` builds one roster in a fresh
process and prints a single `ProcSample` JSON line; that is the measured
workload. The default parent mode re-execs itself `--repeat` times per roster
size for independent cold samples, aggregates them, writes the raw JSON
report (`--out`) and prints a summary. The sampling and aggregation logic
lives in `openhuman_core::platform::proc_metrics`; this binary is the fixture
and process driver.

```bash
cargo build --release -p openhuman-cli --features rss-bench --bin rss-bench
```

### library-profile

Hermetic, Rust-only profiling workloads that run production code paths in
fresh processes, with network inference replaced by a deterministic provider
(`library_profile/mock.rs`). `harness.rs` holds the measurement plumbing and
the pinned output schema (`harness::ProfileResult`). Each scenario is a
module under `library_profile/scenarios/`, selected as
`library-profile <scenario>`:

| Scenario | Measures |
| --- | --- |
| `agent-turn` | A single cold agent turn, the minimal library unit. |
| `long-agent` | N warmed sequential turns with a per-turn checkpoint series. |
| `workflow` | A real flows trigger, transform, agent graph, end to end. |
| `fleet` | N live agents: marginal RSS, idle CPU, fd and thread growth, turn latency. |
| `skill-run` | A skill step on a real `node` child: process-tree RSS. |
| `subagent-storm` | K parallel `agent_memory` subagents in one instance: marginal RSS per subagent. |

stdout is always one pretty-printed JSON object; every diagnostic goes to
stderr with the `[library-profile]` prefix. `OPENHUMAN_PROFILE_WORKER_THREADS`
pins the tokio worker count (set `2` to simulate a 2 vCPU box). With
`rss-bench-dhat`, dhat's global allocator and profiler are active: RSS and
time numbers are perturbed, the result carries `"dhat": true`, and a
`dhat-<scenario>.json` heap profile is written under
`target/profile/rust-library/` (override with `OPENHUMAN_PROFILE_DHAT_OUT`).

The `memory-ingest` and `cold-phases` scenarios went away with the in-process
memory engine (openhuman#6161). Bringing them back means measuring the memory
module over the bus, which is a different scenario (see `scenarios/mod.rs`).

The driver scripts under `scripts/profile/` build it with
`cargo build --release --features rss-bench --bin library-profile`
(`library-heap.sh` uses `--features rss-bench-dhat`). The slim recipe from
`docs/library-benchmarking.md` (`library-bench.sh --slim`) builds both
benchmark binaries without the contributor defaults:

```bash
cargo build --release -p openhuman-cli --no-default-features --features rss-bench \
  --bin library-profile --bin rss-bench
```

## Boundaries

- The `openhuman-core` entry point is `src/main.rs`, one directory up, not
  here.
- Profiling drivers and result handling live in `scripts/profile/`; the
  pure RSS sampling code lives in the core (`platform::proc_metrics`).
- The rankers being measured live elsewhere: BM25 and the `ToolRanker` trait
  in `tinytools`, overlap ranking in `tinyagents-harness`, the Jev ranker in
  `crates/openhuman-tinyhumans/src/jev/`.

## Gotchas

- The manifest sets `autobins = false`, so a `.rs` file in this directory is
  a binary only when it has a `[[bin]]` entry. That is what lets
  `fleet_tests.rs` and `rss_bench_tests.rs` sit here beside their binaries
  without Cargo trying to build them as executables. A new binary needs both
  the file and the manifest entry.
- `tool-search-bench`'s `jev` row and `tool-dialect-bench` make real network
  calls. Keep them out of CI.

## Tests

`fleet_tests.rs` (edge and core token handling, routing) and
`rss_bench_tests.rs` sit beside their binaries, and
`library_profile/scenarios/` has `fleet_tests.rs` and
`subagent_storm_tests.rs`. They build with the binary's required features.
`test-mcp-stub` is exercised by the MCP suites listed above.

```bash
cargo test -p openhuman-cli --features bin-tools --bin openhuman-fleet
cargo test -p openhuman-cli --features rss-bench --bin rss-bench --bin library-profile
```

## See also

- [`docs/library-benchmarking.md`](../../../../docs/library-benchmarking.md):
  the benchmark environment, driver scripts and results for `rss-bench` and
  `library-profile`.
- [`scripts/profile/README.md`](../../../../scripts/profile/README.md): the
  driver scripts.
- [`gitbooks/developing/performance.md`](../../../../gitbooks/developing/performance.md):
  the numbers these benchmarks feed.
- [`gitbooks/developing/jev.md`](../../../../gitbooks/developing/jev.md): what
  `tool-search-bench` compares.
