# Memory scenarios

Realistic memory flows against the real OpenHuman core, on two engines, checked
by the script itself. Every failed check becomes a finding for a person to
read; **nothing is fixed during a run**.

```bash
# both engines (local first, then Built-in on your real account)
node scripts/memory-scenarios/run.mjs   # builtin reads its API key from ~/.memscen-key

# one engine, some scenarios
node scripts/memory-scenarios/run.mjs --engine local --only A,B,E

# keep the local CortexDB volume for a look afterwards
node scripts/memory-scenarios/run.mjs --engine local --keep

# re-render the report of an old run
node scripts/memory-scenarios/run.mjs --grade-only target/memory-scenarios/<run-id>
```

Requires a **debug** core (`cargo build -p openhuman-cli --bin openhuman-core`,
debug so the Composio base override is honoured), Docker, and for the local
engine's real embeddings a running Ollama with `nomic-embed-text` and
`llama3.2:3b` (pulled automatically when missing).

## Headless, but shaped like the desktop app

Same rig as [`../life-scenarios`](../life-scenarios/README.md): the core runs
as `openhuman-core serve` with **its own `HOME`** under
`target/memory-scenarios/<run-id>/<engine>/home`, and turns go through
`openhuman.channel_web_chat` with the reply on `GET /events`, exactly like the
composer. Memory RPCs (`memory_learn`, `memory_items_list`, `memory_recall`,
`memory_brain_ingest`, `memory_sources_*`, `memory_import_*`,
`memory_migration_*`) are called the way the Memory page calls them.

## The two engines

**local** — a throwaway CortexDB **v0.10.5** per run (`cortexdb/` here): its own
compose project `memscen-<run>`, its own volume, a free port. It never touches
the user's `cortexdb` container, its `cortexdb-data` volume or port 3141, and is
torn down with its volume at the end (`--keep` keeps the volume). Inference is
the local Ollama (semantic embeddings, `nomic-embed-text`, 768 dims; a small
chat model for extraction and answers). When Ollama is unreachable it falls back
to tinymemory's deterministic mock inference, and the report says the
recall-quality checks were skipped. The core signs in with an offline local
session and points memory at the container (`memory_engine_set`).

**builtin** — **your real account**, on its managed route. Read this before
running it:

- The account's TinyHumans API key is read from `~/.memscen-key` (or
  `MEMSCEN_KEY_FILE`) at spawn time and reaches the core only as
  `OPENHUMAN_BACKEND_API_KEY`, which the core seeds at boot
  (`security/credentials/ops/boot_env.rs`). It is never an RPC argument, never
  printed, never written to the run directory: `rpc.jsonl` and `core.log` are
  scrubbed of it and of anything `tiny_…` or JWT-shaped.
- **Nothing of the account's own is moved, erased or imported.** Three guards,
  any failure aborts the builtin run before a scenario starts:
  1. the scheduler gate is pre-written **off** in the core's config before it
     first boots (an API key activates no user dir), and read back, so no background job (the layout
     migration's tick, import resume, belief builds) runs on its own;
  2. this workspace's layout-migration state is written as done (`cleaned`)
     and read back through `memory_migration_status`;
  3. no v1 store exists in the throwaway workspace.
     Migration, import, erase and disconnect-with-clear scenarios run **local
     only**. At the end the run asserts the migration state never moved and no
     import ran.
- **Everything the run writes is removed.** Each item carries a run marker
  (`thread-<uuid>` threads, a `memscen:<run>` tag); every id it stores is
  recorded; teardown forgets exactly those, plus every item in the run's
  threads and every item with its tag, then checks each is gone. Survivors are
  reported as a finding. Beliefs the engine derives on its own from the run's
  items cannot be traced back to the run and are not cleaned up.

## Scenarios

| id            | engines | what it checks                                                                                                                                                                                                                                                                           |
| ------------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A-storing     | both    | chats with two agents logged under each agent; learnings of every kind, kind visible on read-back; brain text; a file from a path holding a username (only the basename may reach the engine); dated facts                                                                               |
| B-recall      | both    | cross-thread recall; a negative control (no invention); a planted learning changes a reply; brain retrieval with citations; pre-turn pack timeouts at the default wait vs 6000 ms (`[memory:hooks] pre_turn timed out` in core.log)                                                      |
| C-dates       | both    | the user time zone; "yesterday" and a named date find the right fact; a Spanish question gets a Spanish reply                                                                                                                                                                            |
| D-connectors  | local   | mock Composio (`mock-composio.mjs`): items under `source:<app>`; GitHub split by `{owner}--{repo}`; an edit replaces the old version; disconnect-with-clear removes only that app; a disconnect during a sync leaves nothing; reconnect syncs; the Gmail sender becomes `observed_actor` |
| E-migration   | local   | legacy items plus a v1 store: import with consent, then organizing starts on its own; a core restart mid-move; layout v3; nothing lost; v1 documents present; chats pooled at `ws:main`; the same recall answer                                                                          |
| F-labels      | local   | thread and agent on every logged turn; phone numbers in plain text; `observed_actor` only with `[memory] observed_actor = true`                                                                                                                                                          |
| G-source-cap  | both    | more than four brain sources: at most four in the pack, the named one included, no Team section                                                                                                                                                                                          |
| H-forget      | both    | a forget removes from list and recall; forgetting nothing reports `forgotten: 0`                                                                                                                                                                                                         |
| I-qa          | both    | belief build (`memory_jobs_run`), backfill state, a new learning listed at once; local: the engine stopped mid-session, is the failure surfaced or swallowed                                                                                                                             |
| J-store-speed | builtin | a synthetic batch stored on the hosted path, timed, with `UNAVAILABLE`/429 counted (no real import)                                                                                                                                                                                      |

Not driven here (said so in the report): a workflow writing memory, workflow
memory kept out of chat, and the unconfirmed-import no-erase guard (unit-tested
in the core).

**Known gap, not a finding:** channel messages are not logged to memory today,
so the channel sender name (#7131) is inert.

## The corpus is fictional

`fixtures/persona.json`: one persona (Jordan Lee `<jordan.lee@example.com>`),
`*.example` domains, `555-01xx` phone numbers. The mock's mail and issues are
invented too. Dates the checks reason about are the real clock's, in the
persona's time zone, because the engine stamps items with the real time.

## Output

```
target/memory-scenarios/<run-id>/
  report.md          per engine: inference, scenario table, findings, notes
  findings.json      [{id, engine, scenario, severity, expected, actual, evidence, basis: READ|INFERRED}]
  checks.json        every check, passed or not
  results.json       measurements (pre-turn timeouts, store speed, migration counts, ...)
  <engine>/core.log  the core's log (scrubbed)
  <engine>/rpc.jsonl every RPC with its result (scrubbed)
  <engine>/scenarios/<id>/transcript.jsonl   every turn: message, reply, tools, time
  local/composio-requests.json               what the mock Composio received
```

A finding's `basis` is **READ** when it was seen (a response, a log line) and
**INFERRED** when it was judged (a reply's wording, a heuristic).
