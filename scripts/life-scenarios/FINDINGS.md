# What the life-scenario suite found

Everything below is reproducible with `node scripts/life-scenarios/run.mjs`.
Run directories referenced by timestamp live under `target/life-scenarios/`.

## The numbers

Six scenarios, the shipping desktop path (`channel_web_chat` + SSE, the
orchestrator agent, approval gate on), BYOK to OpenRouter.

**`deepseek/deepseek-v4.1-flash`** (run `2026-09-22T20-10-53`):

| scenario | done | tools | in | cached | out | cost | latency |
| --- | --- | --- | --- | --- | --- | --- | --- |
| calendar-buffer | 0/1 | 0 | 5.7k | 94% | 94 | $0.0040 | 14.0s |
| subscription-scan | 0/1 | 0 | 5.7k | 0% | 69 | $0.0181 | 7.4s |
| baggage-policy | 0/1 | 0 | 5.6k | 0% | 104 | $0.0185 | 8.4s |
| meal-plan | 0/2 | 0 | 5.6k | 0% | 78 | $0.0181 | 11.7s |
| trip-itinerary | 0/1 | 0 | 5.7k | 94% | 136 | $0.0047 | 10.2s |
| fact-check-publish | 0/3 | 0 | 5.7k | 0% | 111 | $0.0188 | 7.0s |
| **total** | **0/9 (0%)** | **0** | 34.1k | 31.5% | 592 | **$0.0822** | 58.7s |

**`anthropic/claude-sonnet-5`** (run `2026-09-22T20-12-36`):

| scenario | done | tools | appr | in | cached | out | cost | latency |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| calendar-buffer | 0/1 | 1 | 0 | 26.8k | 57% | 3.7k | $0.0546 | 42.6s |
| subscription-scan | 9/11 | 4 | 4 | 49.8k | 91% | 1.4k | $0.0483 | 30.4s |
| baggage-policy | 0/1 | 21 | 0 | 298.4k | 39% | 7.2k | $0.6880 | 147.4s |
| meal-plan | 3/10 | 11 | 7 | 224.5k | 54% | 33.0k | $0.7971 | 290.5s |
| trip-itinerary | 0/1 | 14 | 9 | 200.0k | 64% | 35.0k | $0.7825 | 444.1s |
| fact-check-publish | 0/3 | 14 | 3 | 142.5k | 86% | 2.5k | $0.1341 | 89.6s |
| **total** | **12/27 (44%)** | **65** | **23** | 941.9k | 58.1% | 82.9k | **$2.5046** | 1044.6s |

Read the two tables together. The cheap model never calls a tool at all — it
narrates and stops, so it is not a measurement of the harness. The strong model
*works*: 65 tool calls, sixteen minutes, $2.50. And **four of six scenarios
produced no output file whatsoever**, after spending $0.69, $0.78 and $0.13 on
three of them. Findings 1 and 1b are why; [`DIAGNOSIS.md`](DIAGNOSIS.md) traces each one to its transcript.

### After finding 1 was fixed

Same model, same six scenarios, after the fixes below
(run `2026-09-23T11-25-37-650Z`):

| scenario | done | tools | appr | in | cached | out | cost | latency |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| calendar-buffer | **8/8** | 9 | 0 | 77.0k | 81.1% | 16.0k | $0.3028 | 146.8s |
| subscription-scan | **11/11** | 6 | 0 | 47.9k | 90.4% | 1.3k | $0.0466 | 14.2s |
| baggage-policy | 0/1 | 27 | 0 | 491.9k | 71.0% | 15.6k | $0.7671 | 192.3s |
| meal-plan | **10/10** | 8 | 0 | 63.2k | 89.9% | 12.6k | $0.2254 | 91.7s |
| trip-itinerary | 0/1 | 22 | 0 | 1,572.0k | 64.4% | 129.9k | $1.9648 | 900.0s |
| fact-check-publish | 0/3 | 31 | 0 | 220.2k | 80.1% | 12.4k | $0.3709 | 263.1s |
| **total** | **29/34 (85%)** | **103** | **0** | 2,472.2k | 68.8% | 187.9k | **$3.6776** | 1608.1s |

**0/9 → 29/34.** The model that made *zero* tool calls in all six scenarios now
makes 103, because there is finally something to call: `policy-blocked` events
are **0**, `file_write` is on the wire (`visible=17`), and no 1-byte placeholder
files are left behind.

The `appr` column is 0 by design, not by omission: the autonomy policy now
defaults to off, so `gate_decision` answers `Allow` for every class and nothing
parks. Set `[autonomy] enabled = true` to measure the supervised arm.

The three still at zero are **finding 1b**, untouched by that fix:
`grep -c "final permitted model call"` is 2 (baggage-policy and
fact-check-publish reach `max_model_calls=15` after real work), and
trip-itinerary hit the suite's own 900s turn timeout.

Note the cache column against finding 8 below — the 0% collapses are gone.

### After finding 1b was fixed, and a note on reading these tables

Two full runs, same model, back to back (`2026-09-23T15-49-06-514Z` and
`2026-09-23T16-15-03-159Z`):

| scenario           | run A           | run B           | baseline above |
| ------------------ | --------------- | --------------- | -------------- |
| calendar-buffer    | 0/1             | **8/8**         | **8/8**        |
| subscription-scan  | **11/11**       | **11/11**       | **11/11**      |
| baggage-policy     | 0/1             | **8/9**         | 0/1            |
| meal-plan          | **10/10**       | 0/2             | **10/10**      |
| trip-itinerary     | **10/11**       | 0/1             | 0/1            |
| fact-check-publish | 0/3             | **12/12**       | 0/3            |
| **total**          | **31/37 (84%)** | **39/43 (91%)** | 29/34 (85%)    |

**Read the totals with care. The denominators are not comparable** — the grader
stops at `output_exists`, so a scenario that writes nothing contributes `0/1`
while the same scenario passing contributes `0/12`. A run can score a higher
percentage by failing earlier.

**And every scenario but `subscription-scan` flipped between A and B.** That is
this model, not this change: `deepseek-v4.1-flash` intermittently ends a turn
without emitting a tool call (calendar-buffer in run A stopped at 10 of 15
calls, mid-sentence, having just said "let me verify the gap math before writing
the output") or emits a malformed one (baggage-policy in run A: `validation
error: tool web_fetch arguments.url is required`, repeated identically until the
turn was stopped). Single-run comparisons on this model are not evidence of
much, in either direction.

What **is** evidence is the behaviour of the turns that actually reached the
cap, which is the only thing finding 1b touches:

| run      | turns hitting `max_model_calls=15` | artifact written |
| -------- | ---------------------------------- | ---------------- |
| baseline | 2 (baggage, fact-check)            | **0 of 2**       |
| isolated | 1 (baggage)                        | **1 of 1** — 9/9 |
| run A    | 1 (trip-itinerary)                 | **1 of 1**       |
| run B    | 2 (baggage, fact-check)            | **2 of 2**       |

Four capped turns since the fix, four artifacts. Before it, none. The three
scenarios that had never once produced a file — baggage-policy, trip-itinerary,
fact-check-publish — have each now done so, and fact-check-publish went straight
from 0/3 to **12/12** (all three of its outputs) on the run where it capped.

Ordered by severity.

---

## 1. The assistant cannot reliably create a file — FIXED

**Severity: high. The cause of four of the six empty scenarios.**

**Fixed** by, in order of how much each mattered: unpacking `file_write` from
the `files` pack (`tools/toolpacks/registry.rs`) so `close_handed_off_packs` can
no longer deny the orchestrator its only file creator; granting the configured
`action_dir` as a `ReadWrite` trusted root in `SecurityPolicy::from_config`;
`strip_quoted_heredoc_bodies` (`policy_command/quoting.rs`), so a `&` in a
`<< 'EOF'` body is data rather than a background operator; an `apply_patch`
create mode (empty `old_string` on a path that does not exist); and
`[autonomy] enabled`, now defaulting to `false`. See the post-fix table above.

Four mechanisms compose so that the orchestrator has no working route to
writing a new file. Full trace, with the transcript for each step, in
[`DIAGNOSIS.md`](DIAGNOSIS.md) — summarised here:

- **`action_dir` is the join base for relative tool paths but is not a
  permitted write root.** `is_resolved_path_allowed_for`
  (`security/policy/path_checks.rs`) allows `workspace_root` or a trusted root;
  `enforcement.rs:118-126` grants a trusted root for `default_projects_dir()`,
  which reads `OPENHUMAN_PROJECTS_DIR` and knows nothing about
  `OPENHUMAN_ACTION_DIR`. On a stock install the two coincide and this is
  invisible. **Change the working folder — `action_dir_override`, which is what
  the Settings control writes — and file-tool writes into it are refused**
  `Resolved path escapes workspace`, for a path inside the directory
  `CLAUDE.md` calls "the agent's permitted read and write root". Verified: also
  granting the dir via `OPENHUMAN_PROJECTS_DIR` took `policy-blocked` events
  from several to zero.
- **`classify_command` rejects `&` inside a quoted heredoc body.** The blocked
  `cat > out/meal_plan.md << 'EOF'` contained four ampersands, every one of them
  in a recipe title ("Greek Chicken **&** Spinach Orzo Skillet"), and was
  refused as command substitution / background. `subscription-scan` wrote
  successfully with the identical heredoc shape and no `&` in its content.
  Ampersands are ordinary in prose, so `shell` is an unreliable writer for the
  documents an assistant produces.
- **`use_skill` cannot reach `file_write`.** Withheld packs are documented as
  reachable through `use_skill`; for the orchestrator the `files` pack is
  *closed* by `close_handed_off_packs` (#6302), and the call returns
  "Skill `files` has no tools available in this session."
- **`apply_patch` cannot create a file** — `` `old_string` must not be empty ``,
  and it canonicalizes the target, so the file must already exist.

`meal-plan` spent 11 rounds and $0.80 cycling through all four and left two
**1-byte files containing `x`** — the placeholder it created so `apply_patch`
would have something to patch.

---

## 1b. A capped turn withdraws every tool — PARTLY FIXED

**Severity: medium.** Three scenarios reached `max_model_calls=15` — and
reached it partly because of the above.

**Fixed:** the half that silently destroyed the deliverable. The reporting half
— a capped turn being indistinguishable from a finished one on the wire — is
still open; see the end of this section.

At the last permitted call, `FinalCallWrapUpMiddleware` clears
`request.tools` (25 of them), sets `tool_choice = None`, and injects
`MAX_ITER_CHECKPOINT_INSTRUCTION`: *"You have reached the maximum number of tool
calls allowed for this single turn … report the substance of what this turn
produced … close with a brief **Still to do** line."* The turn is then
structurally unable to write anything, and the model correctly produces a
status report.

The mechanism is deliberate (#6014) and right for a turn whose product is
*text*. It is exactly wrong for one whose product is a **file**: the model
arrives at that call holding the finished content and has, structurally,
nowhere to put it.

`baggage-policy` is the clean demonstration. 27 tool calls of research, a
correct summary of Delta's published limits in the reply — and
`out/delta_baggage_guide.md` never written, so every grader check after
`output_exists` was unreachable and it scored 0/1. The reply's own closing line
was *"Still to do: … write `out/delta_baggage_guide.md`"*.

**The fix.** One call earlier the belt is *narrowed* to the tools that can only
write (`DELIVERABLE_TOOLS` = `file_write`, `apply_patch`) rather than left
whole, with its own instruction saying gathering is over and that a file marking
its own gaps honestly beats no file. `shell` is deliberately excluded: it can
redirect into a file but can equally run a crawler, so keeping it would leave
the belt effectively unnarrowed. The restoration of microcompact-cleared results
that the conclusion already got now runs here too — a call asked to write
findings down has to be able to read them.

The trade is explicit: **a capped turn spends its second-to-last round
persisting rather than gathering**, and a turn with nothing to persist loses
that round. That is the same trade the final call already made, moved one step
earlier. The alternative — leaving the belt intact and the instruction advisory
— is what `FinalCallWrapUpMiddleware`'s own doc comment records as having
already failed.

Guarded to `max_model_calls >= 3` (below that, reserving would leave no round in
which anything could be gathered to write) and to belts that actually carry a
writer (telling a read-only agent that "the only tools left are the ones that
write files" would simply be false).

Measured, on the same scenario and model that produced the 0/1:

```
model_calls=14 max_model_calls=15 tools_withdrawn=24 tools_kept=2
   -> file_write  out/delta_baggage_guide.md  (4734 bytes, tool call 31 of 32)
model_calls=15 max_model_calls=15 tools_withdrawn=26
   -> the conclusion, as before
```

**9/9, 100%**, 12 delta.com URLs cited. The turn still hit the cap — the
reservation is precisely what turned it from a zero into a pass. The guide also
marks the transatlantic Basic Economy fee "NOT STATED" rather than inventing it,
which is what the prompt asked for and what the old run never got far enough to
do.

**Still open:** the other half of this finding — **the caller cannot tell.** `turn_run_finalize.rs:210` computes `hit_cap` and
`flows/ops/builder.rs:336` consumes it, but `grep hit_cap` over `web_chat/`
finds nothing and `TurnUsagePayload` (`core/socketio.rs:353`) has no cap field.
On the path the desktop app uses, a truncated turn arrives as an ordinary
`chat_done` — indistinguishable from a complete answer except by reading the
prose.

---

## 2. `main` does not build — three vendor gitlinks point at non-ancestor commits

**Severity: critical. Nothing in Rust compiles on `main`.**

Commit `f74efad5d2` ("chore(deps): update vendor submodules for tinyagents,
tinyjuice, and tinymcp") moved all three submodules to commits that are **not
ancestors of their own upstream `main`**, and each is missing a symbol
OpenHuman references:

| submodule | pinned at | missing | referenced from |
| --- | --- | --- | --- |
| `vendor/tinyagents` | `6c3105e6` (side branch) | the whole `tinyagents-runtime` crate | `crates/openhuman-core/Cargo.toml:148` |
| `vendor/tinyjuice` | `2f02042` (release tag) | `tinyjuice_bus::types::ReadIntent`, `compressors::html::html_to_markdown` | `inference/tokenjuice/schemas.rs:314`, `tools/impl/network/web_fetch.rs:250` |
| `vendor/tinymcp` | `8b0627d` | `ConnectedServerOverview::instructions` | `agent/registry/agents/orchestrator/prompt.rs:437` |

The `tinyagents` one fails at *manifest resolution*, so it is not a compile
error you can work around — `cargo check`, `cargo build`, `cargo test` and
`cargo metadata` all fail before any code is read:

```
error: failed to get `tinytools-jev` as a dependency of package `openhuman-cli`
  unable to update .../vendor/tinyagents/vendor/tinytools/crates/tinytools-jev
```

The previous pin (`8582277`, tinyagents upstream `main`) satisfies all three.
This branch repins `tinyagents` → `8582277`, `tinyjuice` → `36e9657`,
`tinymcp` → `fe34f5b`, all upstream `main`, and the workspace builds.

**Worth a CI gate**: a `cargo metadata --no-deps` on every submodule bump would
have caught this in seconds, and a check that each gitlink is an ancestor of
its upstream default branch would have caught the cause.

---

## 3. The replayed transcript carries no assistant tool calls

**Severity: medium — confirmed for the persisted transcript, inferred for the
live request.** See [`DIAGNOSIS.md`](DIAGNOSIS.md) for what is measured and
what is not.

Every assistant turn that made a tool call is persisted with **empty content
and no `tool_calls`**, and the tool result comes back as a plain `user`
message. From
`workspace/tinyagents_store/journal/session.*_orchestrator_ls-subscript.messages.jsonl`:

```
0  system    len=20638
1  system    len=467
2  user      len=1117    the task
3  assistant len=0       tool_calls=false   <-- the tool call is gone
4  user      len=806     "[Tool results] <tool_result id=...-model-1-tool-1>"
5  assistant len=0       tool_calls=false   <-- and again
6  user      len=4112    "[Tool results] ...-model-2-tool-1"
...
```

So from round two on, the model is looking at a conversation in which results
arrive for requests it cannot see it made. It has to infer its own actions from
their output.

A careful model notices and refuses. `calendar-buffer` on
`anthropic/claude-sonnet-5` read the calendar successfully and then answered:

> I wasn't able to complete this task. [...] I also have no tool record showing
> `out/meeting_buffer_plan.json` was actually written — no `apply_patch` or
> file-write call appears in what ran.

That is a correct reading of the transcript it was given. (It also asserted the
file read had been truncated, which was **not** true — the tool result in the
journal is the complete 1,821-byte file. The structural complaint was right;
that particular detail was a hallucination.)

Result: one tool call, two model calls, `max_iterations=15` never approached,
and no output file. Scenarios that happen to be linear enough survive it —
`subscription-scan` scored 9/11 with the same transcript shape — but anything
needing the model to verify its own work does not.

Per `CLAUDE.md`, tool-call dialects and transcript replay belong to
`vendor/tinyagents`, so **the fix is an upstream tinyagents-harness change**,
not a host-side patch.

---

## 4. Every provider call is written to the cost log twice

**Severity: medium. The cost dashboard reports 2× tokens and 2× requests.**

Six model calls across a six-scenario run produced **twelve** rows in
`workspace/state/costs.jsonl` — each call once as `provider_charged` with the
real amount and once as `estimated` with `cost_usd: 0.0`, same tokens, same
second:

```
rows: 12      duplicate (in,out,second) groups: 6 of 6
cost_source:  {'provider_charged': 6, 'estimated': 6}
sum tokens:   69,392        (actual: 34,696)
```

Two independent call sites record the same call:

- `agent/tinyagents/observability/event_bridge.rs:401`
- `agent/tinyagents/host/budget_gate.rs:292` (the `BudgetGate::record` impl)

`turn_outcome.rs:110-113` documents the intended invariant — *"The bridge and
this fallback are mutually exclusive, so spend is recorded exactly once either
way"* — and the third site (`turn_run_finalize.rs:175`) does honour it. The
budget gate is not covered by that reasoning.

`total_cost_usd` survives only by luck: the duplicate happens to price at
`0.0` (see finding 4). `request_count` and `total_tokens` are simply doubled.

---

## 5. A model missing from the pricing catalog is billed at $0

**Severity: medium. Budget enforcement silently stops working.**

`cost::catalog::estimate_cost_usd` returned `0.0` for
`deepseek/deepseek-v4.1-flash` — a current, chargeable OpenRouter model. The
budget gate's own comment says pricing there is *"deliberately not skipped,
because a zero-cost ledger would silently disable `check_budget` enforcement
altogether"* — which is exactly what an unknown model produces.

An unknown model should price at a conservative fallback, not zero.
`FALLBACK_PRICING` already exists in `agent/cost.rs:47-53`; the catalog path
does not reach it.

---

## 6. `config.update_model_settings` accepted a BYOK config it could not honour — FIXED on this branch

**Severity: medium. Every subsequent turn fails.**

The field's own schema comment says:

> `inference_url` — Custom OpenAI-compatible LLM endpoint. **When set together
> with `api_key`, inference goes direct to this URL** instead of the OpenHuman
> backend.

Setting exactly those two succeeds. The next turn then dies:

```
[chat-factory] BYOK_INCOMPLETE: inference_url is set to a custom/direct
endpoint (https://openrouter.ai/api/v1) but no matching cloud_providers entry
was found for role 'chat'.
```

`provider_for_role` resolves through `cloud_providers`, never through
`inference_url`. The caller had to *also* hand-build a `cloud_providers` entry
whose endpoint matched and pin each agent-turn role to `<slug>:<model>` — a
failure in a different subsystem, one call later, for a write the API accepted,
and none of it discoverable from the error.

**Fixed** by `complete_byok_route` in
`crates/openhuman-core/src/config/ops/model.rs`: when `inference_url` and
`api_key` both arrive non-blank and no entry matches the endpoint, register one
and pin the four roles an agent turn runs on — the same completion
`config/schema/ephemeral_route.rs::apply` already does for a single call. It is
deliberately narrow: both halves required, an existing entry for that endpoint
reused rather than duplicated, a blank `default_model` declined (the grammar is
`<slug>:<model>`), and any role the same patch pinned — or that already points
somewhere deliberate like `ollama:…` — left alone.

Covered by `config/ops/model_byok_tests.rs` (9 tests), and end-to-end by
`run.mjs`, which now sends only the three documented fields and routes.

---

## 7. The default agent has no file tools on the wire

**Severity: medium — a capability gap, and arguably intended.**

A default orchestrator turn advertises 16 tools:

```
[agent] tool spec filter: total=232 visible=16 names=[shell, apply_patch,
spawn_async_subagent, list_subagents, continue_subagent, todo, resolve_time,
memory_store, memory_recall, update_memory_md, goal_complete, http_request,
web_fetch, web_search_tool, composio_connect, use_skill]
[toolpacks] withheld packed tool schemas; use_skill advertised instead hidden=23
```

No `file_read`, `file_write`, `grep`, `glob` or `list`. `ToolGroups::default()`
puts every pack in `Withheld`, so reading a file means `shell` (`cat`) or a
`use_skill` round-trip. That is a deliberate prompt-budget decision and it is
documented — but for the single most common assistant task, "read this file and
write that one", it costs an extra hop and pushes work into a shell the sandbox
then has to police.

`--driver rpc --agent life_scenarios` runs the same scenarios with the file
tools named explicitly, for comparison.

---

## 8. Prompt-cache hit rate collapses between turns

**Severity: medium. 4× cost difference, same work.**

Six consecutive turns on `deepseek/deepseek-v4.1-flash`, identical ~21 KB
system prompt, each a fresh thread:

| scenario | input | cached | cost |
| --- | --- | --- | --- |
| calendar-buffer | 5.7k | **94%** | $0.0040 |
| subscription-scan | 5.7k | 0% | $0.0181 |
| baggage-policy | 5.6k | 0% | $0.0185 |
| meal-plan | 5.6k | 0% | $0.0181 |
| trip-itinerary | 5.7k | **94%** | $0.0047 |
| fact-check-publish | 5.7k | 0% | $0.0188 |

The uncached turns cost **4.5×** the cached ones for the same prompt. Part of
this is provider-side routing (OpenRouter can move a request between backends,
and `served_by` drift breaks the prefix cache), which is why the repo ships
`scripts/debug/capture-first-inference.mjs` — its `cache_key` / `served_by`
lines are the right next instrument here.

### The collapses are gone, and that is a measurement artefact worth naming

Same six scenarios after the finding-1 fixes
(run `2026-09-23T11-25-37-650Z`):

| scenario | input | cached | cost |
| --- | --- | --- | --- |
| calendar-buffer | 77.0k | 81.1% | $0.3028 |
| subscription-scan | 47.9k | **90.4%** | $0.0466 |
| baggage-policy | 491.9k | 71.0% | $0.7671 |
| meal-plan | 63.2k | 89.9% | $0.2254 |
| trip-itinerary | 1,572.0k | **64.4%** | $1.9648 |
| fact-check-publish | 220.2k | 80.1% | $0.3709 |
| **total** | 2,472.2k | **68.8%** | **$3.6776** |

No 0% rows, and the floor is 64.4% against an overall 31.5% before. **Do not
read that as the caching bug being fixed** — nothing here touched inference
routing. The honest reading is that the baseline was measuring something else:
those turns were one or two model calls long (0 tool calls, ~5.7k input), so a
single cold call *was* the whole scenario and one `served_by` miss showed up as
0%. These turns are 6–31 tool calls over 48k–1.6M input tokens, so a cold first
call is amortised across many warm ones and the per-scenario number is dominated
by the steady state.

What that does establish is the steady state itself, which the baseline could
not: the prefix cache **does** hold across the turns of one thread, at 64–90%.
The open question from this finding is unchanged and still needs the capture
proxy — whether `cache_key` stays identical and `served_by` stops drifting
*within* a thread. The first call of each scenario is still cold, and at these
prompt sizes that is now the expensive part.

---

## 9. A turn with no hosted session retries a failing backend call ~3× per turn

**Severity: low in the product. Was severe in this rig — see below.**

With no hosted session, every orchestrator roster build calls
`GET /agent-integrations/composio/toolkits`, gets 401, and logs it:

```
[backend_api] 401 on GET /teams/me/usage — session token rejected
[integrations] backend rejected session JWT (401) path=/agent-integrations/composio/toolkits
[composio] fetch_connected_integrations: list_toolkits (backend) failed: SESSION_EXPIRED
[orchestrator_tools] assembled 20 delegation tool(s) ... (0 integrations connected)
```

That block repeats three times per turn — the roster is rebuilt once per
`build`, and nothing negative-caches the 401 for the life of the turn. Measured
cost: ~3 s of a 5 s pre-inference window.

**The same 401 was not low-severity for `web_search_tool`, and that was the
rig's fault.** `web_search_tool` posts to `/agent-integrations/parallel/search`
on the same hosted backend, so it too came back `SESSION_EXPIRED` every time —
but it was advertised to the model anyway (`tool spec filter: … web_search_tool`
among 17 visible). So the model spent calls discovering the tool was dead and
then routed around it by hand: in the 2026-09-23 run `baggage-policy` burned two
calls on the 401s, improvised a DuckDuckGo HTML scrape, guessed delta.com paths
and collected four 404s. That is most of how it reached the cap in finding 1b.

Offering a capability the run's own configuration cannot serve is a defect in
the benchmark, not the product. `mock-search.mjs` now serves that one route
locally, **mocking discovery but not retrieval**: it ranks a fixture corpus of
*real* URLs the agent still fetches over the network for itself, so the
`cites_delta_com` and dimension checks stay honest. The README covers why it
takes over the whole backend base, and the two things that were easy to get
wrong — the per-user config dir race (`api_url` in `config.toml` loses to
`users/<runtime-id>/config.toml`, so the override rides `BACKEND_URL`) and the
`{ success, data }` envelope `parse_envelope` requires (a bare payload fails as
`missing field 'success'`, which reads to the agent as a broken tool rather than
an empty result — it gave up after six tries).

---

## 10. Smaller things

- **`AgentDefinition.system_prompt` rejects the spelling its own error
  suggests.** `system_prompt = "..."` fails to parse, because `PromptSource`
  deserializes as an externally-tagged enum and the accepted form is the table
  `[system_prompt] inline = '''...'''`. The validation error you get instead
  reads *"missing `system_prompt` — custom definitions must set an inline
  string or a file path"*, which describes the spelling that does not work.
- **`approval.decide` param errors are fully redacted.** Calling it with `id`
  instead of `request_id` logs `param-validation error (message redacted;
  skip-report)` and nothing else — no field name, no expectation. A headless
  caller sees only that approvals silently stop being granted while the turn
  parks.
- **`direct` Composio needs three things together, two undocumented.**
  `[composio] mode = "direct"` in config, **both**
  `OPENHUMAN_COMPOSIO_DIRECT_BASE_V2` and `..._V3` (the match arm is
  `(Some, Some)`; one alone silently falls through to production URLs), and a
  **debug** build (the override is `#[cfg(debug_assertions)]`-gated in
  `integrations/composio/client/factory.rs:93`). See this directory's README.
- **`composio_list_tools` returns `{"tools":[]}` in direct mode** by
  short-circuit (`integrations/composio/tools/list_tools.rs:152-168`), so an
  agent on a direct connection cannot discover what it may call.

## 11. Two things about this machine, not the codebase

- The operator's `~/.openhuman/config.toml` has
  `api_url = "http://127.0.0.1:18473"`, a capture proxy that is not running, so
  the desktop app's backend calls currently all fail.
- `~/.openhuman/dev-keychain.json` holds dozens of keys prefixed
  `.tmpXXXXXX:` (`.tmpobNPzo:auth:openai:default`, …) — test temp workspaces
  leaking into the operator's real keychain rather than their own.
