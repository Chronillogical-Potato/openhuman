# openhuman-embed

`openhuman-embed` is the library crate a product depends on to run the
OpenHuman core inside its own process. It turns the core's JSON controllers
and ambient task-locals into ordinary Rust types: build one `Runtime`, put any
number of `Agent`s on it, and run turns that return a typed `TurnOutcome`.
OpenCompany and other embedding products use it directly, and
`openhuman-tinyhumans` wraps it to add the hosted TinyHumans backend. The
narrative walkthrough is
[`gitbooks/developing/embedding.md`](../../gitbooks/developing/embedding.md).

## How it works

The crate has two entry points, and they sit at different heights over the
core.

```text
   host product
        |
        |  Runtime::builder() ... build()        Core::from_runtime(rt)
        v                                              |
  +-----------+   agent(spec)   +---------+            |
  |  Runtime  | --------------> |  Agent  | (n per     |
  +-----------+                 +---------+  runtime)  |
        |                            |                 |
        | CoreBuilder::build         | Turn::send      | Config / Auth /
        v                            v                 | CoreAgent
  +----------------------------------------------+     |
  | openhuman-core (package `openhuman`)          | <---+
  |  CoreRuntime, CoreContext, controller registry|
  +----------------------------------------------+
```

The library API is `Runtime` then `Agent`. `RuntimeBuilder::build` claims the
process slot, resolves a workspace on disk, assembles one base `Config`,
writes the TinyHumans API key into the credential store (when given), and
hands everything to the core's `CoreBuilder`. The result is one booted
`CoreRuntime` wrapped in a `Runtime`.

`Runtime::agent(spec)` then turns an `AgentSpec` into an `Agent`. Nothing runs
and no RPC is made. `agent::build::instantiate` validates the id, clones the
runtime's base config, and applies the spec on top in a fixed order: access,
provider model, MCP servers, Composio credential, memory binding, then the
`config` escape hatch. It creates the agent's directories, copies its skills,
builds its `AgentDefinition`, checks that the agent only narrows the runtime's
`DomainSet` and `ToolGroups`, and finally derives a per-agent `CoreContext`
with `CoreContext::derive_with`.

A turn on that agent goes through `Turn::send`:

```text
 agent.turn("msg").session(id).send()
   |
   |- mint a session id ("embed-<uuid>") if none was given
   |- refuse a half route, or a bearer sent to a non-https endpoint
   |- enter the origin and progress task-local scopes, if set
   |- runtime.run_in(agent ctx, ...)
   |     agent_chat_for(config, AgentChatTarget::Definition { .. }, ...)
   |       (inference::host_runtime::ops, native, no JSON)
   v
 TurnOutcome { reply, session_id, usage }
```

Because the turn runs under the agent's own context, the core's config
loader, `DomainSet` gate, tool-group filter and skill discovery all read that
agent's settings. Two agents on one runtime never see each other's provider,
access tier, MCP servers or skills. Conversations are keyed by thread id and
agent id, so a turn only resumes its own agent's thread.

The second entry point is `Core`, for a host that built a `CoreRuntime` with
`CoreBuilder` itself. `Core::from_runtime` wraps it and exposes typed
sub-facades (`config()`, `auth()`, `agent()`). These go through
`CoreRuntime::invoke` (the private `call` helper in [`src/call.rs`](src/call.rs)), so they
honour `DomainSet` gating and absorb the variable `{result, logs}` envelope
that controllers emit. `CoreAgent` runs the runtime's orchestrator over the
`inference.agent_chat` RPC. `Core::raw()` is the escape hatch for a call the
facade does not model yet; a raw call that keeps coming back is a candidate
for a typed method here.

## Using it

Add the dependency with the default contributor features, or pick a narrow
host build:

```toml
[dependencies]
openhuman-embed = { git = "https://github.com/tinyhumansai/openhuman", package = "openhuman-embed" }

# or
openhuman-embed = { git = "https://github.com/tinyhumansai/openhuman", package = "openhuman-embed", default-features = false, features = ["inference", "mcp"] }
```

### A runtime and several agents

For hosted TinyHumans inference and services, build the runtime with
[`openhuman_tinyhumans::RuntimeBuilder`](../openhuman-tinyhumans/README.md).
It mirrors this crate's builder and installs the SDK-backed backend transport
on `build()`. The plain `openhuman_embed::RuntimeBuilder` suits standalone
hosts that bring their own providers.

```rust,no_run
use openhuman_tinyhumans::{
    embed::{Access, AgentSpec, McpServer, Provider, Workspace},
    RuntimeBuilder,
};

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let runtime = RuntimeBuilder::new()
    .workspace(Workspace::dir("/var/lib/my-product/openhuman"))
    .api_key("th_live_...")                   // the only credential in library mode
    .build()
    .await?;

let reviewer = runtime.agent(
    AgentSpec::new("reviewer")
        .system_prompt("You review pull requests and never edit files.")
        .access(Access::readonly())
        .skills_dir("./skills/review")        // copied into this agent's skills root
        .action_dir("/srv/checkouts/pr-42"),
)?;

let fixer = runtime.agent(
    AgentSpec::new("fixer")
        .provider(Provider::openai_compatible("https://api.example/v1", "sk-...").model("gpt-5"))
        .access(Access::full())
        .mcp(McpServer::stdio("github", "gh-mcp", ["stdio"]))
        .action_dir("/srv/checkouts/pr-42"),
)?;

let review = reviewer.run("Summarise the risks in this change.").await?;
let fix = fixer
    .turn(format!("Address these findings:\n{}", review.reply))
    .send()
    .await?;

// Continue the same conversation with the same agent.
let again = fixer.turn("Now run the tests.").session(&fix.session_id).send().await?;
println!("{}", again.reply);
# Ok(())
# }
```

`McpServer` and `AgentSpec::mcp` need the `mcp` feature; `skills_dir` needs
`skills`. `Agent::run` always starts a new conversation. To continue one, pass
the returned `session_id` to `Turn::session`.

A `Turn` has more knobs than the example shows: `model` and `temperature`
for one turn, `cwd` to root the file and shell tools somewhere else (use
`openhuman_embed::absolute` for a relative path, since the core resolves a
relative `cwd` against `action_dir`), `route` for a one-off endpoint,
`origin` for the authority the turn claims, `on_progress` for live
`AgentProgress` events, `meter` for usage that is reported even when the turn
fails, and `seed` to replace the session's history with rows the host
supplies.

### Harness: the one-agent shorthand

`Harness` is a `Runtime` plus exactly one `Agent` with the id `harness`,
built from one set of inputs. `harness.runtime()` and `harness.agent()` hand
out the two halves, so a host that outgrows one agent adds more on the same
runtime instead of building a second harness.

```rust,no_run
use openhuman_embed::{Access, Harness, Provider, Workspace};

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let harness = Harness::builder()
    .provider(Provider::openai_compatible("https://api.example/v1", "sk-...").model("gpt-5"))
    .workspace(Workspace::Ephemeral)
    .access(Access::readonly())
    .build()
    .await?;

let first = harness.run("Summarize what you can see.").await?;
let second = harness
    .turn("Now list the risks.")
    .session(&first.session_id)
    .send()
    .await?;
println!("{}", second.reply);
# Ok(())
# }
```

A harness keeps a few older defaults. Its agent's `action_dir` is the
workspace's own `action/` directory, its skills are copied into the shared
`<workspace>/skills` root, and it only turns on the `mcp` and `skills` domain
families when servers or a skills directory were supplied. See
[`src/harness/README.md`](src/harness/README.md).

### The typed core facade

```rust,no_run
use std::sync::Arc;

use openhuman_embed::{Core, CoreBuilder, DomainSet, HostKind, ServiceSet};

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let runtime = CoreBuilder::new(HostKind::Library)
    .domains(DomainSet::embedded())
    .services(ServiceSet::none())
    .build()
    .await?;
let core = Core::from_runtime(Arc::new(runtime));
let flags = core.config().runtime_flags().await?;
println!("log_prompts={}", flags.log_prompts);
# Ok(())
# }
```

`Core::agent()` needs the `inference` domain family at runtime. Note that
`DomainSet::harness()` leaves `inference` off despite its name; use
`DomainSet::embedded()`.

### Product identity

An embedding product sets its identity once at startup, before any backend
client is built. It becomes the `x-sdk-name` header on TinyHumans requests.

```rust,no_run
use openhuman_tinyhumans::{set_product_identity, ProductIdentity};

if let Some(identity) = ProductIdentity::new("opencompany") {
    set_product_identity(identity);
}
```

`openhuman_tinyhumans::RuntimeBuilder::product_identity` does the same as
part of the build.

## What the runtime owns and what an agent owns

The runtime owns everything that is process-scoped in the core: the
workspace and credential store, the event bus, the keyring, the RPC bearer,
the background `ServiceSet`, the registered `DomainSet` and the API key.
Agents can only narrow the domain set, so a runtime has to register `mcp` and
`skills` if any agent will use them. The default runtime does when those
features are compiled in.

Each agent owns its provider route and model, its access tier and turn
origin, its `action_dir`, its MCP servers, its skills root, its
`AgentDefinitionSpec` (system prompt, tool scope, sandbox mode, iteration cap,
temperature), its narrowed `DomainSet` and `ToolGroups`, its memory binding
and an optional host tool factory.

The on-disk layout under a runtime-owned root:

```text
<root>/
  config.toml, auth-profiles.json, core.token     runtime-wide
  workspace/
    session_db/sessions.db                        runtime-wide run ledger
    session_raw/                                  transcripts (thread + agent)
    agents/<id>/skills/                           each agent's skills root
  agents/<id>/action/                             default action_dir per agent
```

The default `action_dir` sits beside the workspace, never inside it, because
the core refuses agent writes beneath the workspace. An inherited workspace
uses `<action_dir>/agents/<id>` instead.

### Settings that are still runtime-wide

Every agent reads these from the runtime's boot config today. Each is a
candidate follow-up in the core.

- `autonomy.auto_approve` and `auto_approve_all`, and the memory guard's
  autonomy tier, come from the boot config (`security::live_policy`). Path
  and command policy do use the agent's own tier.
- The approval gate is on or off for the whole process, and parked approvals
  are not labelled with an agent id. For an agent that never waits on
  approval, use `Access::full()`, whose `TrustedAutomation` origin the gate
  honours per turn.
- The sub-agent catalogue is runtime-wide (built-ins plus
  `<workspace>/agents/*.toml`). Embedded agents cannot be `delegate_*`
  targets of each other. Do not reuse built-in ids such as `orchestrator` or
  `summarizer` for your agents.
- Sub-agents an agent spawns, and the experience store, read the runtime's
  on-disk config rather than the agent's overlay.
- Sub-agent run-ledger rows, cron jobs and the cost log stay in the
  workspace even with a host session store.
- Agents on one workspace share the dynamic `mcp_registry_*` MCP registry.
  Servers declared through `AgentSpec::mcp` are per agent. The host-seeded
  documentation server is visible to every agent.
- `install_skill` and `create_skill` still write to `~/.openhuman`. With
  `include_user_skills(false)` (the default) an agent does not discover the
  operator's skills, but an install it performs lands there.
- One API key (or session) is shared by every agent.

## Authentication and the backend

Library mode has no user login. `RuntimeBuilder::api_key` stores a
TinyHumans API key in the runtime's credential store before the core boots.
Managed inference then sends it as `Authorization: Bearer <key>`, backend
REST calls send it as `x-api-key`, and the scheduler gate treats the runtime
as signed in. There is no `/auth/me` round trip and no session to expire. An
agent that names its own `Provider` never uses the key for inference.

The key covers the hosted features the core reaches: managed inference,
cloud embeddings, voice (STT and TTS), web search, media generation, the Jev
ranker, Composio and the other `/agent-integrations/*` tools, referral and
webhooks. Callers that can only send a bearer (the vendored STT and embedding
clients, the connector module's proxy route, the memory engine's sync) send
the key as `Authorization: Bearer`, which the backend accepts by its
`tiny_live_` / `tiny_test_` prefix. What a key may reach is decided by its
scopes on the backend (`inference`, `voice`, `search`, `media`, `storage`,
`account`, `connections`); a missing scope answers `403`, and a key minted
through the grant flow omits `connections` unless it was asked for. The
session-bound `/auth/*` flows (OAuth connect, channel link tokens, login
tokens), the realtime voice agent and the Socket.IO relay still need a
signed-in user. See
[`gitbooks/developing/tinyhumans-api-key.md`](../../gitbooks/developing/tinyhumans-api-key.md).

A host that drives backend features for a signed-in user passes a `Session`
to `RuntimeBuilder::session` (or `HarnessBuilder::session`, or later
`Auth::store`). The core stores it through `openhuman.auth_set_credential`
and never validates it. Obtaining and checking the JWT is the host's job,
which `openhuman_tinyhumans::session` does for the desktop app and TUI.
`Session::local` builds the offline `.local` token form, which authorizes
nothing at the backend.

The core reaches the hosted backend only through a `BackendTransport`
(re-exported here). This crate installs none. Without one, agents, memory,
skills, tools and RPC still work, and hosted surfaces (managed inference,
billing, integrations tools, cloud voice, channel relay) answer with a typed
`BACKEND_UNAVAILABLE:` error. `openhuman_tinyhumans::RuntimeBuilder` installs
the SDK transport for you; `RuntimeBuilder::backend_transport` takes your own.

## Memory per tenant

Bind an agent with
`AgentSpec::memory(MemoryBinding::new(agent_id).root("team:acme"))` and its
turns run TinyMemory's lifecycle under `team:acme/agent:<agent_id>`.
`Runtime::memory("team:acme")` returns a `memory::Memory`, the operator's view
of that tenant: its agents, items, learnings and brain documents, with every
call confined to the root's subtree. `RuntimeBuilder::memory_engine` installs
a host-supplied engine in place of the configured one. See
[`docs/specs/memory-v2.md`](../../docs/specs/memory-v2.md).

## Conversations in a host store

By default an agent's transcripts, turn journal, run status, goals and todos
are files under the workspace. A host serving many users from one process
keeps them in its own database instead:

```rust,ignore
let runtime = Runtime::builder()
    .workspace(Workspace::stateless())          // nothing durable on disk
    .session_store(Arc::new(MyMongoStores::new(db)))
    .build()
    .await?;
```

The runtime asks the provider (`session_store::SessionStoreProvider`) for each
agent's stores by agent id, so a provider over a shared database scopes every
query by agent. A reopened agent resumes its thread from the store, in any
process. `InMemorySessionStores` keeps everything in memory, and the
conformance suites re-exported from `session_store` are what a host's
provider is held to. `Workspace::stateless()` refuses to build without a
store (`RuntimeError::NoSessionStore`); a private scratch directory, removed
with the runtime, still holds process-local caches. The desktop app, CLI and
TUI install `openhuman_rpc::session_store`, the classic on-disk layout behind
the same port.

## Tools on an agent

`AgentSpec::tools` gives an agent the host's own in-process tools, each with
its own schema on the wire. The factory runs once per session build, which
in practice is once per turn, and receives a `TurnContext` naming the agent
and the session. `Agent::attach_tools` adds a permanent named tool source to
an agent that already exists, which is how one library hands a configured
agent to another. See [`src/agent/README.md`](src/agent/README.md).

## Layout

| Path | What it does |
| --- | --- |
| [`src/lib.rs`](src/lib.rs) | Re-exports (core runtime types, `Tool`, `ToolGroups`, the backend port, `session_store`, `agent_progress`) and `Core`, the typed facade over a caller-built `CoreRuntime`. |
| [`src/runtime/`](src/runtime/README.md) | `Runtime`, `RuntimeBuilder`, `ApiKey`, and the `CoreGuard` that owns process-scoped state until the last handle drops. |
| [`src/agent/`](src/agent/README.md) | `Agent`, `AgentSpec`, `AgentDefinitionSpec`, `AgentLayout`, `MemoryBinding`, the build step, and permanent tool attachments. |
| [`src/harness/`](src/harness/README.md) | `Harness` and `HarnessBuilder`, plus the shared input types: `Access`, `Provider`, `Workspace`, `McpServer`, skill copying, `HarnessError`. |
| [`src/turn.rs`](src/turn.rs) | `Turn`, `TurnRequest`, `TurnOutcome`, `Route`: one turn, its ambient scopes, and its dispatch to an agent or to the orchestrator. |
| [`src/call.rs`](src/call.rs) | The private typed dispatch helper over `CoreRuntime::invoke` that every facade method uses. |
| [`src/config.rs`](src/config.rs), [`src/auth.rs`](src/auth.rs), [`src/core_agent.rs`](src/core_agent.rs) | The `Core` sub-facades: runtime flags, credentials (`Session`, `AuthState`), and the orchestrator turn. |
| [`src/memory.rs`](src/memory.rs) | `Memory`, the per-tenant memory facade returned by `Runtime::memory`. |
| [`src/error.rs`](src/error.rs) | `CoreError`, the error every facade call returns (`Domain`, `Unavailable`, `Rpc`, route refusals). |
| [`examples/`](examples/README.md) | Runnable programs: one turn on a harness, and two agents on one runtime. |
| [`tests/`](tests/README.md) | End-to-end suites against `wiremock` providers. |

## Key types and entry points

- `Runtime` and `RuntimeBuilder` ([`src/runtime/`](src/runtime/)): build once per process,
  then call `Runtime::agent`. `Runtime::core()` gives non-turn access
  (config, auth) without exposing a way to start a turn.
- `AgentSpec` ([`src/agent/spec.rs`](src/agent/spec.rs)): the pure-data description of an agent.
  Ids must match `^[a-z0-9][a-z0-9_-]{0,63}$`.
- `Agent` ([`src/agent/mod.rs`](src/agent/mod.rs)): a cheap clone handle. `run`, `turn`,
  `attach_tools`, and path accessors (`action_dir`, `home_dir`,
  `skills_dir`, `transcripts_dir`).
- `Turn` and `TurnOutcome` ([`src/turn.rs`](src/turn.rs)): configure, then `send`.
- `Access` ([`src/harness/access.rs`](src/harness/access.rs)): the autonomy tier and turn origin,
  always set together. Presets are `readonly`, `supervised` (the default)
  and `full`.
- `Provider` ([`src/harness/provider.rs`](src/harness/provider.rs)): `openai_compatible(base_url, key)`
  plus `model`, or `inherit()` for the machine's configured inference.
- `Workspace` ([`src/harness/workspace.rs`](src/harness/workspace.rs)): `Ephemeral` (default), `Dir`,
  `Stateless`, or `Inherit`.
- `Harness` ([`src/harness/mod.rs`](src/harness/mod.rs)): one runtime, one agent.
- `Core` ([`src/lib.rs`](src/lib.rs)): the typed facade over a caller-built runtime.

## One model call, no runtime: `Completer`

A host that needs *a completion*, not an agent — a code reviewer asking for a
JSON verdict, a classifier, an extractor — uses `complete::Completer`. It needs
no `Runtime`, so none of the one-per-process and worker-stack constraints above
apply, and any number of calls can run concurrently.

```rust,no_run
# async fn demo() -> Result<(), openhuman_embed::CoreError> {
use openhuman_embed::complete::{ChatMessage, CompletionRequest, Completer, ResponseFormat};
use openhuman_embed::Route;

let completer = Completer::new(Route::openai_compatible("https://openrouter.ai/api/v1", "sk-…"))
    .header("X-Title", "my-app");
let response = completer
    .complete(
        CompletionRequest::new("openai/gpt-5-mini", vec![ChatMessage::user("Is 7 prime?")])
            .response_format(ResponseFormat::JsonObject)
            .max_tokens(64)
            .provider_options(serde_json::json!({"usage": {"include": true}})),
    )
    .await?;
println!("{:?} {:?} {:?}", response.structured, response.finish_reason, response.usage);
# Ok(())
# }
```

It differs from an agent turn on purpose: no prompt-injection guard (callers
pass untrusted text *as data*), no tools (a request declaring tools is
refused), no session or orchestrator prompt, and no model fallback — the
requested model is sent verbatim and `answered_model` reports what the
provider says actually ran. `finish_reason`, token usage (cached and reasoning
included) and the gateway-reported cost come back on the response; a
`CompletionObserver` sees every call, success or failure, for trace export.

`openhuman_embed::embeddings` re-exports the embedding models (OpenAI, Voyage,
Cohere, Ollama, mock) for hosts that keep their own vector index, with the
signature format unchanged so stored vectors stay in their partition.

## Locked-down agents: `HostOnly`

An agent that reads untrusted input (a PR diff) and must never act takes
`ToolScopeSpec::HostOnly`: the tools it can see and call are the ones the host
supplies (`AgentSpec::tools`, `Agent::attach_tools`) and nothing else.

```rust,no_run
use openhuman_embed::{AgentDefinitionSpec, AgentSpec, HostTurnTools, ToolScopeSpec};
# fn read_only_tools() -> Vec<Box<dyn openhuman_embed::Tool>> { Vec::new() }
let spec = AgentSpec::new("reviewer")
    .definition(
        AgentDefinitionSpec::new()
            .bare_prompt("You review pull requests.")
            .tools(ToolScopeSpec::HostOnly),
    )
    .tools(|_| HostTurnTools::advertised(read_only_tools()));
```

- No config-derived tool is built and no delegation tool is synthesised, so
  shell, file writes, network, memory, skills, MCP and sub-agents do not exist
  for the model. A deny-by-default gate refuses any other name it calls anyway.
- Access and sandbox are forced read-only, whatever `Access` was set (even
  after `AgentSpec::config`). Declaring MCP servers or skills is an error.
- The agent needs its own prompt. `bare_prompt(text)` makes `text` the whole
  system prompt: no identity, safety, tools, workspace or memory section.

Host tools should themselves be read-only: `HostOnly` bounds which tools
exist, not what they do.

Per turn, `Turn::response_format(ResponseFormat)` (the `complete` type) and
`Turn::max_tokens(n)` apply to every call of the tool loop on any runtime-owned
agent. `TurnOutcome` then carries `structured` (the reply parsed as JSON),
`finish_reason`, `answered_model`, and `usage.reasoning_tokens`.
`Turn::untrusted_input(true)` reads the message as data, so the prompt-injection
guard and screen are skipped. It is accepted only on a `HostOnly` agent; any
other agent refuses the turn (`untrusted_input_requires_host_only`).

## Feature flags

Every feature is a pass-through to the same-named feature on
`openhuman-core` (package `openhuman`): `default`, `http-server`, `inference`,
`documents`, `hosting`, `modules`, `voice`, `web3`, `runtime-node`, `media`,
`flows`, `skills`, `mcp`, `crash-reporting`, `channels`, `whatsapp-web`,
`file-logging` and `scheduler-gate`. A gate added to the core has to be
forwarded here (and then by `openhuman-tinyhumans` and `openhuman-cli`);
[`scripts/ci/check-feature-forwarding.mjs`](../../scripts/ci/check-feature-forwarding.mjs) checks the chain.

Two features also gate this crate's own surface. `mcp` adds `HttpHeader`,
`McpAuthConfig`, `McpServer`, `AgentSpec::mcp` and `HarnessBuilder::mcp`.
`skills` adds `AgentSpec::skills_dir` and `HarnessBuilder::skills_dir`.

[`docs/library-minimal-recipe.md`](../../docs/library-minimal-recipe.md) has a
measured minimal feature set, and
[`gitbooks/developing/performance.md`](../../gitbooks/developing/performance.md)
has the resulting binary sizes and per-agent memory.

## Boundaries

- The only in-repo dependency is `openhuman-core` with
  `default-features = false`. Every capability comes from a forwarded
  feature.
- No JSON-RPC here. This crate does not depend on `openhuman-rpc`; `Outcome`
  and `StructuredRpcError` are core types. `openhuman-app` and
  `openhuman-tui` use `openhuman-rpc` and the core directly and do not use
  this crate.
- No backend client. The hosted transport, product identity, login-token
  exchange and `/auth/me` live in `openhuman-tinyhumans`.
- The agent loop, transcripts, session identity and tool-call parsing belong
  to [`vendor/tinyagents`](../../vendor/tinyagents/) (repo `tinyhumansai/tinyagents`); the `Tool` trait
  belongs to its nested `tinytools`. Use the `Tool` re-exported here: a
  separately added `tinytools` builds an incompatible type.
- Memory contracts come from [`vendor/tinymemory`](../../vendor/tinymemory/) (`tinymemory-api`).

## Gotchas

- One runtime per process. The keyring master key, the RPC bearer, the
  event bus and the `Once`-guarded domain subscribers are process-scoped
  (`CoreContext::init`). A second `RuntimeBuilder::build` returns
  `RuntimeError::AlreadyRunning`; a second harness returns
  `HarnessError::AlreadyRunning`. `Core::from_runtime` is not guarded, but
  the same rule applies to the `CoreRuntime` beneath it. The slot is
  released only when the `Runtime` and every `Agent` built on it are
  dropped.
- Build the tokio runtime yourself. A turn is a deep async state machine,
  and a nested sub-agent overflows tokio's default 2 MiB worker stack, so
  `#[tokio::main]` is not enough. The constants live in the core crate, so
  the host needs the `openhuman` package as a dependency to name them:

  ```rust,no_run
  use openhuman_core::core::runtime::{AGENT_WORKER_STACK_BYTES, MAX_BLOCKING_THREADS};

  let runtime = tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .thread_stack_size(AGENT_WORKER_STACK_BYTES)
      .max_blocking_threads(MAX_BLOCKING_THREADS)
      .build()
      .expect("tokio runtime");
  ```

- Access has two halves. The autonomy tier drives `SecurityPolicy`, and the
  turn origin (a task-local) drives the approval gate. `Access::full()` sets
  both; setting only the tier through `config` gives an agent whose acting
  tools refuse while the transcript looks plausible. `readonly()` and
  `supervised()` set no origin and leave the approval gate on, so an
  unattended supervised agent stalls on approvals until they expire.
- When building a `CoreRuntime` yourself, set `config_path` together with
  `workspace_dir` (`CoreBuilder::workspace(dir)` does both). Credentials,
  auth profiles and the keyring file resolve beside `config_path`, so a
  workspace-only override reads the operator's real credentials. `Runtime`
  and `Harness` handle this for `Ephemeral`, `Stateless` and `Dir`, and the
  `AgentSpec::config` escape hatch cannot move it.
- Supply skills through `skills_dir`, which copies bundles. Skill discovery
  rejects symlinked bundles, so linking them in silently does nothing.
- `Provider::model` is advisory. A model no provider serves falls back to
  the default rather than failing, and a route with no model resolved is
  ignored.
- A `Route` (or `Provider::openai_compatible`) that pairs a key with a plain
  `http://` endpoint is refused before dispatch with
  `CoreError::InsecureRoute`. A route without a key is allowed.
- A resumed session keeps its first system prompt and tool list. Prompt
  changes and a tool factory that returns a different belt only reach new
  sessions.

## Tests

The integration suites under [`tests/`](tests/README.md) each build one
runtime per process against `wiremock` providers; unit tests sit beside
their modules as `*_tests.rs`.

```bash
cargo test -p openhuman-embed --features inference,mcp,skills
cargo test -p openhuman-embed --features inference,mcp,skills --test runtime_agents
```

The repository-root [`examples/embed_headless.rs`](../../examples/embed_headless.rs) and [`examples/embed_kernel.rs`](../../examples/embed_kernel.rs)
drive `CoreBuilder` directly, without this crate. They are `[[example]]`
targets of `openhuman-cli`:
`cargo run -p openhuman-cli --example embed_headless`.

## Further reading

- [`crates/openhuman-embed/examples/README.md`](examples/README.md): the examples module README.
- [`gitbooks/developing/embedding.md`](../../gitbooks/developing/embedding.md): embedding the core in another product.
- [`gitbooks/developing/tinyhumans-api-key.md`](../../gitbooks/developing/tinyhumans-api-key.md): running on a TinyHumans API key.
- [`gitbooks/developing/architecture.md`](../../gitbooks/developing/architecture.md): architecture overview.
- [`gitbooks/developing/architecture/agent-harness.md`](../../gitbooks/developing/architecture/agent-harness.md): the agent harness.
- [`gitbooks/developing/loadable-modules.md`](../../gitbooks/developing/loadable-modules.md): loadable modules.
- [`crates/README.md`](../README.md): crates overview.
