# Diagnosis: why four of six scenarios produced no file

A forensic trace of the 2026-09-22 runs, from the artifacts in
`target/life-scenarios/`. `FINDINGS.md` is the list of defects; this is the
causal account, with the evidence for each step and an explicit note where I am
inferring rather than measuring.

**The short version.** The headline number — four of six scenarios wrote
nothing while spending $0.69, $0.78 and $0.13 — is not one failure. It is three,
and the one I originally blamed (the iteration cap) is the *least* important of
them. The dominant cause is that **the orchestrator has no working way to create
a file**, and it burns its iteration budget discovering that.

---

## The measurement that started it

`anthropic/claude-sonnet-5`, run `2026-09-22T20-12-36-423Z`, from
`grep "turn prompt summary" core.log` (parent turns only; rows 5 and 6 have
`system_bytes=5130` and are sub-agent runs spawned by the `research` tool):

| # | scenario | model_calls | tool_calls | outcome |
| --- | --- | --- | --- | --- |
| 1 | calendar-buffer | 2 | 1 | stopped early, **not** capped |
| 2 | subscription-scan | 5 | 4 | finished naturally — 9/11 |
| 3 | baggage-policy | **15** | 20 | **cap** |
| 4 | meal-plan | 12 | 11 | finished naturally, wrote 1-byte files |
| 7 | trip-itinerary | **15** | 14 | **cap** |
| 8 | fact-check-publish | **15** | 14 | **cap** |

The cap is `max_model_calls = max_iterations = 15`
(`agent/tinyagents/turn_policy.rs:139`) — a ceiling on **model calls**, not tool
calls, which is why row 3 shows 20 tool calls under a cap of 15.

`grep -c "final permitted model call" core.log` → **3**, matching rows 3, 7 and
8 exactly. So three of the four empty scenarios hit the cap and one (meal-plan)
did not. calendar-buffer is a fourth, separate failure.

---

## Cause 1 — the orchestrator cannot create a file (meal-plan, and the reason the others ran out)

This is the important one, and it is fully visible in one transcript. From
`meal-plan`'s journal, in order, with every tool result:

| step | tool | result |
| --- | --- | --- |
| 1 | `shell` `mkdir -p out` | ok |
| 2 | `apply_patch` | `` edit[0]: `old_string` must not be empty `` |
| 3 | `shell` heredoc write | `Blocked: [policy-blocked] … command/process substitution ($(…), <(…)), backticks, and background (&) are not allowed` |
| 4 | `apply_patch` | `arguments.edits[0].path is required` |
| 5 | `shell` | `Command failed (exit code 2)` |
| — | *(harness injects)* | `[no progress since step 5] 4 tool calls in a row have failed…` |
| 6 | `shell` | ok |
| 7 | `apply_patch` | `Blocked: [policy-blocked] Resolved path escapes workspace: …/sandbox/meal-plan/out/meal_plan.md` |
| 8 | `shell ls` | orienting |
| 9 | `apply_patch` | `Failed to resolve path 'out/meal_plan.md': No such file or directory` |
| 10 | `shell pwd` | orienting |
| 11 | `apply_patch` | `Failed to resolve path 'meal_plan.md': No such file` |

Eleven rounds, $0.80, and the two files it left behind are **one byte each,
containing `x`** — the placeholder `shell` created at step 1 so that
`apply_patch` would have something to patch.

Four separate mechanisms combine to close every door:

### 1a. `file_write` is not on the wire

`[agent] tool spec filter: total=232 visible=16` — the 16 are `shell`,
`apply_patch`, the sub-agent tools, `todo`, `resolve_time`, memory tools,
`http_request`, `web_fetch`, `web_search_tool`, `composio_connect`, `use_skill`.
No `file_write`, `file_read`, `grep`, `glob` or `list`: `ToolGroups::default()`
puts every pack in `Withheld` (`tools/toolpacks/groups.rs`).

### 1b. The `use_skill` escape hatch is closed for exactly the pack that would help

Withheld is documented as "reachable via `use_skill`". In the follow-up run the
agent tried precisely that — `use_skill {"skill":"files"}` — and got:

> Skill `files` has no tools available in this session. Call `plan` or
> `run_code` or `review_code` or `run_skill` instead — that agent owns these
> tools and runs them directly. Do not retry this skill.

That is `tools/toolpacks/tools.rs:206` firing because `found == 0`: every tool
in the pack was made non-callable by `close_handed_off_packs`
(`agent/session_host/builder/builder_build.rs`, #6302) — *"a pack whose owner
this agent can hand off to directly is that specialist's belt, not this
agent's"*. So for the orchestrator the file pack is not withheld-but-reachable,
it is **closed**, and the only route is delegating to a specialist the
orchestrator was not told to use for "write this file".

### 1c. `apply_patch` cannot create a file

`` `old_string` must not be empty `` and `Failed to resolve path …: No such
file or directory` — it canonicalizes the target, so the file must already
exist. Combined with 1a/1b this is the whole write surface, and it is
edit-only.

### 1d. `shell` refuses to write content containing `&`

This is the sharpest defect of the run. The blocked command at step 3 was a
quoted heredoc:

```
cat > out/meal_plan.md << 'EOF'
# 5-Day Mediterranean Dinner Plan (Family of 4)

## Day 1 — Greek Chicken & Spinach Orzo Skillet
...
```

Scanning that command string for the characters the classifier objects to finds
four hits, all of them the same thing:

```
'&' at 106  -> '## Day 1 — Greek Chicken & Spinach Orzo Skillet'
'&' at 2034 -> '## Day 3 — Spinach & Feta Stuffed Chicken…'
'&' at 3044 -> '## Day 4 — Mediterranean Chickpea & Spinach Skillet'
'&' at 3835 -> '## Day 5 — Shrimp & Vegetable Mediterranean Pasta'
```

No `$(`, no backtick, no `<(`. The command was refused because **recipe titles
contain an ampersand**. `classify_command` reads the raw command string, so a
`&` inside a single-quoted heredoc body — where the shell treats it as literal
text — is indistinguishable to it from a background operator.

That `subscription-scan` succeeded with the *same* `cat > … << 'EOF'` shape is
the control: heredocs are fine. Its content simply had no `&` in it.

Prose contains ampersands constantly — "Q&A", "R&D", "Chicken & Spinach", any
URL with a query string, any HTML entity. This makes `shell` an unreliable
writer for exactly the documents an assistant produces.

The model noticed and adapted, rewriting the titles to "Chicken and Spinach" on
its next attempt — which then failed on 1c instead.

### 1e. `action_dir` is the join base but not a permitted root

Step 7's `Resolved path escapes workspace:
…/sandbox/meal-plan/out/meal_plan.md` is about a path **inside**
`OPENHUMAN_ACTION_DIR`, which `CLAUDE.md` calls "the agent's permitted read and
write root".

`security/policy/path_checks.rs`:

```rust
// validate_path: relative paths are joined onto action_dir …
let full_path = … self.action_dir.join(&expanded) …

// …but the permission check never mentions action_dir:
pub fn is_resolved_path_allowed_for(&self, resolved: &Path, require_write: bool) -> bool {
    if Self::is_always_forbidden(resolved) { return false; }
    let workspace_root = self.workspace_root_sync();
    resolved.starts_with(&workspace_root) || self.is_within_trusted_root(resolved, require_write)
}
```

The write permission comes from a trusted root, and
`security/policy/enforcement.rs:118-126` grants one for
**`default_projects_dir()`** — which reads `OPENHUMAN_PROJECTS_DIR` and knows
nothing about `OPENHUMAN_ACTION_DIR`
(`config/schema/load/dirs.rs:78-90`).

On a stock install `action_dir == default_projects_dir`, so this never shows.
**Change the action dir — via `OPENHUMAN_ACTION_DIR`, or `action_dir_override`,
which is what the Settings working-folder control writes — and the file tools
lose write permission to the directory the agent is told to work in.**

**Verified experimentally.** Re-running `meal-plan` with
`OPENHUMAN_PROJECTS_DIR` additionally pointed at the same sandbox took
`grep -c "policy-blocked" core.log` from several to **0**. The agent still
failed — on 1b/1c/1d, which the grant does not touch — but the path rejection
was gone. `run.mjs` now sets both variables and says why.

---

## Cause 2 — the cap withdraws every tool and asks for a report (baggage-policy, trip-itinerary, fact-check-publish)

At model call 15 of 15, `FinalCallWrapUpMiddleware`
(`agent/tinyagents/middleware/final_call_wrap_up.rs`) fires:

```
[tinyagents::mw] final permitted model call — withdrawing tools and asking for
the turn's conclusion  model_calls=15 max_model_calls=15 tools_withdrawn=25
```

It clears `request.tools`, sets `tool_choice = None`, and appends
`MAX_ITER_CHECKPOINT_INSTRUCTION` (`agent/session_host/turn_checkpoint.rs:85`):

> You have reached the maximum number of tool calls allowed for this single
> turn, so you cannot call any more tools right now. Do not attempt another
> tool call.
>
> First, report the substance of what this turn produced … Then close with a
> brief **Still to do** line naming what remains, so the user can ask you to
> carry on.

The model complied exactly. baggage-policy:

> Here's what I found from Delta's actual pages (I haven't written the file yet
> — running low on tool calls this turn, so reporting findings first)

**I was wrong in my first pass** to describe this as the model spontaneously
noticing a budget. It was told, in those words, by the harness. There is no
"running low on tool calls" string anywhere in the codebase — that phrasing is
the model paraphrasing the injected instruction.

The mechanism is deliberate and well-built (#6014 replaced a worse out-of-band
wrap-up). The problem is the situation it is asked to rescue: with all 25 tools
withdrawn, **the turn is structurally incapable of writing the file**, however
much budget was spent getting ready to. And these three scenarios only reached
call 15 because they spent rounds on Cause 1.

### The caller is never told

`turn_run_finalize.rs:210` computes `hit_cap`, and it does reach some consumers
— `flows/ops/builder.rs:336` reads `last_turn_hit_cap`. But `grep hit_cap` over
`web_chat/` returns nothing, and `TurnUsagePayload` (`core/socketio.rs:353`)
carries `input_tokens`, `output_tokens`, `cached_input_tokens`, `cost_usd`,
`context_window`, `subagents` — and no cap flag.

So on the path the desktop app uses, a capped turn arrives as an ordinary
`chat_done`. Nothing machine-readable distinguishes "here is your answer" from
"I ran out of room and this is a status report". This suite only found out by
reading the reply prose.

---

## Cause 3 — calendar-buffer refused on transcript grounds (2 model calls, no cap)

Separate from both. The agent ran `cat calendar-buffer/calendar/calendar.json`,
got the **complete** 1,821-byte file back (verified: the tool result in the
journal ends with `ev-07`, `]`, `}`), and then answered:

> I wasn't able to complete this task. The only tool result I have is a
> truncated read of `calendar/calendar.json`, cut off after the third event
> … I also have no tool record showing `out/meeting_buffer_plan.json` was
> actually written — no `apply_patch` or file-write call appears in what ran.

The truncation claim is false — the file was complete. **The second claim is
true**, and it is the interesting half. From the journal:

```
0  system    len=20638
1  system    len=467
2  user      len=1117   the task
3  assistant len=0      tool_calls=false   <-- no record of the call
4  user      len=806    "[Tool results] <tool_result id=…-model-1-tool-1>"
5  assistant len=0      tool_calls=false
6  user      len=4112   "[Tool results] …-model-2-tool-1"
```

Every assistant turn that called a tool is stored with empty content and no
tool-call array; results come back as `user` messages. The same shape appears in
`subscription-scan`, which succeeded anyway — so this degrades reliability
rather than guaranteeing failure.

`message_convert.rs:549-563` **preserves** tool calls when they are present
(`a.tool_calls.is_empty()` → plain chat, else `AssistantToolCalls`), so their
absence from the journal means they were absent from `run.messages`, the live
array. The dialect is text-based — the system prompt teaches
`<tool_call>\nread_file(path="…")\n</tool_call>` — so the model's call lived in
message *text* that was parsed out and not retained.

**Where this stops being measurement.** `session_raw` for the same turn *does*
contain an assistant record carrying `tool_calls`, so the two stores disagree
and the information is not lost everywhere. Whether the live provider request
also lacked the calls is the one thing I could not confirm: I put the repo's
capture proxy in front of the route
(`CAPTURE_ALL=1 … CAPTURE_UPSTREAM=https://openrouter.ai`) and the core
**silently fell back to the managed backend** rather than dial a plain-`http://`
loopback inference URL — `egress-surface emitting external_transfer_pending
provider=openhuman` despite `[config][byok] pinned role(s) [chat, reasoning,
agentic, coding] to 'byok-inference:…'` two seconds earlier. That silent
downgrade is its own defect, and fixing it is the prerequisite for settling this
question.

Until then: the model's behaviour is *consistent* with the live request lacking
its own tool calls, and the journal *proves* a resumed thread would lack them.
I would not claim more than that without the wire capture.

---

## What I got wrong in the first pass

Worth stating plainly, since the point of this document is a diagnosis that
holds up:

1. **I called the iteration cap the primary cause.** It is third. Three
   scenarios hit it, but they hit it *because* of Cause 1, and meal-plan and
   calendar-buffer never approached it.
2. **I said the harness gives the model no budget warning and the model noticed
   on its own.** The opposite: `MAX_ITER_CHECKPOINT_INSTRUCTION` tells it in
   almost exactly those words.
3. **I repeated Sonnet's claim that the tool result was truncated.** It was not.
   The model hallucinated that detail; only its structural complaint held up.
4. **I described the missing tool calls as a straightforward replay bug.** The
   journal shape is confirmed; the live-request claim is not, and the capture
   run that would have settled it failed for an unrelated reason.

---

## Ranked, with the fix each one wants

| # | defect | evidence | fix belongs in |
| --- | --- | --- | --- |
| 1 | `action_dir` is not a permitted write root unless it equals `default_projects_dir` | step 7 block; `enforcement.rs:118-126`; grant experiment took blocks 3→0 | `security/policy/enforcement.rs` — grant `config.action_dir`, not just the default |
| 2 | `classify_command` rejects `&` inside a quoted heredoc body | four `&` hits, all recipe titles; `subscription-scan` control | command classifier — parse enough shell to know a quoted heredoc body is data |
| 3 | the `files` pack is closed to the orchestrator, so `use_skill` cannot reach `file_write` | `use_skill {"skill":"files"}` → "no tools available in this session" | `close_handed_off_packs` / orchestrator prompt — either keep a writer, or teach delegation before the model has failed four times |
| 4 | `apply_patch` cannot create a file | `` `old_string` must not be empty ``; `No such file or directory` | give it a create mode, or advertise a writer |
| 5 | a capped turn is indistinguishable from a finished one on the wire | `hit_cap` absent from `web_chat/` and `TurnUsagePayload` | `web_chat/presentation.rs` + `TurnUsagePayload` |
| 6 | plain-`http://` loopback BYOK silently downgrades to the managed backend | `provider=openhuman` after a successful `[config][byok]` pin | provider factory — honour loopback http, or fail loudly |
| 7 | the replayed transcript carries no assistant tool calls | journal dumps above | `vendor/tinyagents` (dialect / transcript replay) |

1 through 4 are one user-visible bug: **the assistant cannot reliably write a
file.** Fixing any one of them individually would have turned some of these
runs green; fixing 1 and 2 would turn most of them green.

---

## Reproducing

```bash
# the three-cause run
node scripts/life-scenarios/run.mjs --model anthropic/claude-sonnet-5

# the cheapest single reproduction of Cause 1
node scripts/life-scenarios/run.mjs --only meal-plan --model anthropic/claude-sonnet-5
grep -c "policy-blocked" target/life-scenarios/<run>/core.log
wc -c target/life-scenarios/<run>/sandbox/meal-plan/out/*
```

The journals that back every quote above:

```
<run>/home/.openhuman/users/*/workspace/tinyagents_store/journal/session.*.messages.jsonl
<run>/home/.openhuman/users/*/workspace/session_raw/*.jsonl   # tool call arguments
```
