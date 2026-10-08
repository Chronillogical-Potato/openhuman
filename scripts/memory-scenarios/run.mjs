#!/usr/bin/env node
// Memory scenarios: realistic memory flows against a real headless core, on a
// throwaway local CortexDB and on Built-in (the user's real account), checked
// by the script itself. Every failed check is a finding for the user; nothing
// is fixed during a run. See README.md.

import fs from "node:fs";
import fsp from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  Core,
  EventStream,
  JsonlLog,
  Findings,
  Ledger,
  sendTurn,
  waitFor,
  withRetries,
  pick,
  randomUUID,
  sleep,
  scrub,
} from "./lib.mjs";
import {
  startLocalCortex,
  activeWorkspace,
  placeMigrationGuard,
  verifyBuiltinUntouched,
  forgetLedger,
  builtinCredential,
} from "./engines.mjs";
import { startMockComposio } from "./mock-composio.mjs";
import { SCENARIOS, registerLocalOnly } from "./scenarios.mjs";
import { connectors } from "./connectors.mjs";
import { migration } from "./migration.mjs";

registerLocalOnly(connectors, migration);

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, "..", "..");
const FIXTURES = path.join(HERE, "fixtures");

function parseArgs(argv) {
  const opts = {
    only: [],
    engine: "both",
    keep: false,
    gradeOnly: "",
    coreBin: path.join(REPO, "target", "debug", "openhuman-core"),
    runRoot: path.join(REPO, "target", "memory-scenarios"),
    settleMs: 4000,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    const next = () => argv[++i];
    if (a === "--only") opts.only.push(...next().split(","));
    else if (a === "--engine") opts.engine = next();
    else if (a === "--keep") opts.keep = true;
    else if (a === "--grade-only") opts.gradeOnly = next();
    else if (a === "--core-bin") opts.coreBin = next();
    else if (a === "--settle-ms") opts.settleMs = Number(next());
    else if (a === "--help" || a === "-h") {
      console.log(
        fs
          .readFileSync(path.join(HERE, "README.md"), "utf8")
          .split("\n")
          .slice(0, 30)
          .join("\n"),
      );
      process.exit(0);
    } else throw new Error(`unknown argument ${a}`);
  }
  if (!["local", "builtin", "both"].includes(opts.engine))
    throw new Error("--engine local|builtin|both");
  return opts;
}

/** The offline local session token (as life-scenarios mints it). */
function mintLocalSessionToken(userId) {
  const b64 = (o) => Buffer.from(JSON.stringify(o)).toString("base64url");
  const now = Math.floor(Date.now() / 1000);
  return [
    b64({ alg: "none", typ: "JWT" }),
    b64({
      sub: userId,
      iat: now,
      exp: now + 86_400,
      email: "memscen@local.invalid",
    }),
    "local",
  ].join(".");
}

/** Set `key = value` lines in a TOML section of `file` (a small, line-based edit). */
async function editToml(file, section, kv) {
  let text = await fsp.readFile(file, "utf8").catch(() => "");
  const lines = text.split("\n");
  const header = `[${section}]`;
  let start = lines.findIndex((l) => l.trim() === header);
  if (start < 0) {
    lines.push("", header);
    start = lines.length - 1;
  }
  let end = lines.findIndex((l, i) => i > start && /^\s*\[/.test(l));
  if (end < 0) end = lines.length;
  for (const [k, v] of Object.entries(kv)) {
    const line = `${k} = ${typeof v === "string" ? JSON.stringify(v) : v}`;
    const at = lines
      .slice(start + 1, end)
      .findIndex(
        (l) => l.trim().startsWith(`${k} =`) || l.trim().startsWith(`${k}=`),
      );
    if (at >= 0) lines[start + 1 + at] = line;
    else {
      lines.splice(end, 0, line);
      end += 1;
    }
  }
  await fsp.writeFile(file, lines.join("\n"));
}

const BASE_CONFIG = (extra = []) =>
  [
    "schema_version = 13",
    "onboarding_completed = true",
    "chat_onboarding_completed = true",
    "",
    "[observability]",
    "analytics_enabled = false",
    "share_usage_data = false",
    "",
    ...extra,
  ].join("\n");

// ---------------------------------------------------------------------------
// one engine
// ---------------------------------------------------------------------------

async function runEngine(engine, { opts, runDir, runId, findings, results }) {
  const dir = path.join(runDir, engine);
  const home = path.join(dir, "home");
  const oh = path.join(home, ".openhuman");
  await fsp.mkdir(oh, { recursive: true });
  const marker = `memscen:${runId}`;
  const secrets = [];
  const extraEnv = {};
  const engineResults = { engine, marker, scenarios: {} };
  results.engines[engine] = engineResults;
  const log = (m) => console.log(m);

  let cortex = null;
  let composio = null;
  let core = null;
  let events = null;
  const ledger = new Ledger();
  const threads = [];

  // --- engine setup -------------------------------------------------------
  if (engine === "local") {
    cortex = await startLocalCortex({
      runId,
      scriptDir: HERE,
      tinymemoryDir: path.join(REPO, "vendor", "tinymemory"),
      keep: opts.keep,
      log,
    });
    secrets.push(cortex.apiKey);
    engineResults.inference = cortex.inference;
    composio = await startMockComposio();
    extraEnv.OPENHUMAN_COMPOSIO_DIRECT_BASE_V3 = composio.url;
    extraEnv.OPENHUMAN_COMPOSIO_DIRECT_BASE_V2 = composio.url;
    await fsp.writeFile(
      path.join(oh, "config.toml"),
      BASE_CONFIG([
        "[composio]",
        'mode = "direct"',
        'api_key = "ck_memscen_mock"',
        'entity_id = "default"',
        "",
      ]),
    );
  } else {
    const cred = await builtinCredential();
    secrets.push(cred.secret);
    Object.assign(extraEnv, cred.env);
    // Guard 0: the scheduler gate is off before the core first boots, so no
    // background job (the migration's tick, import resume, belief builds)
    // runs on its own. An API key is seeded as a plain profile write with no
    // user-dir activation (boot_env.rs), so the root config is the active
    // one. Verified after boot; the run aborts if it is not in effect.
    await fsp.writeFile(
      path.join(oh, "config.toml"),
      BASE_CONFIG(["[scheduler_gate]", 'mode = "off"', ""]),
    );
  }

  const rpcLog = new JsonlLog(path.join(dir, "rpc.jsonl"), secrets);
  const logFile = path.join(dir, "core.log");
  const startCore = async () => {
    core = new Core({ coreBin: opts.coreBin, rpcLog, secrets });
    const health = await core.start({
      home,
      actionDir: path.join(dir, "action"),
      logPath: logFile,
      extraEnv,
    });
    events = new EventStream(core, `memscen-${runId.slice(-8)}`);
    await events.connect();
    return health;
  };

  try {
    const health = await startCore();
    log(`${engine.padEnd(8)}: core ${core.url} pid ${health.pid}`);

    if (engine === "local") {
      const uid = "memscen-local";
      await core.rpc("openhuman.auth_set_credential", {
        token: mintLocalSessionToken(uid),
        kind: "local",
        userId: uid,
        user: { _id: uid, email: "memscen@local.invalid", name: "Jordan Lee" },
      });
      await withRetries(
        async () => {
          await core.rpc("openhuman.memory_engine_set", {
            engine: "cortexdb",
            endpoint: cortex.endpoint,
            api_key: cortex.apiKey,
          });
          const got = await core.rpc("openhuman.memory_engine_get", {});
          if (got?.engine !== "cortexdb" || got?.status !== "ok")
            throw new Error(
              `engine ${got?.engine} ${got?.status} ${got?.reason ?? ""}`,
            );
        },
        { attempts: 20, delayMs: 1000, what: "local CortexDB engine" },
      );
      // config.toml now lives in the activated user dir: the composio block too.
      const { config } = await activeWorkspace(core);
      const snap = await core.rpc("openhuman.config_get", {});
      const cfgPath = snap?.config_path;
      if (cfgPath && pick(config, "composio.mode") !== "direct") {
        await editToml(cfgPath, "composio", {
          mode: "direct",
          api_key: "ck_memscen_mock",
          entity_id: "default",
        });
        await core.stop();
        await startCore();
      }
    } else {
      // Guard 0 verified: the gate the account's config carries is off.
      const snap = await core.rpc("openhuman.config_get", {});
      const mode = pick(snap, "config.scheduler_gate.mode");
      if (mode !== "off")
        throw new Error(
          `ABORT builtin: scheduler gate is "${mode}", not "off" (user dir not the one pre-written)`,
        );
      const eng = await core.rpc("openhuman.memory_engine_get", {});
      if (eng?.engine !== "tinyhumans" || eng?.status === "off")
        throw new Error(
          `ABORT builtin: memory engine is ${eng?.engine}/${eng?.status} (${eng?.reason ?? ""}); is the session valid?`,
        );
      // Guard 1: the migration of this workspace counts as done.
      const { workspace } = await activeWorkspace(core);
      await placeMigrationGuard(
        core,
        path.join(workspace, "memory", "layout_migration.json"),
      );
      const imp = await core.rpc("openhuman.memory_import_scan", {});
      if (imp?.found)
        throw new Error(
          "ABORT builtin: a v1 store is present in the throwaway workspace",
        );
      engineResults.inference = { kind: "managed (account route)" };
      log(
        `builtin : guards in place (scheduler gate off, migration state cleaned, no v1 store)`,
      );
    }

    // --- scenarios ----------------------------------------------------------
    const selected = SCENARIOS.filter((s) => s.engines.includes(engine))
      .filter(
        (s) =>
          !opts.only.length ||
          opts.only.some((o) => s.id.toLowerCase().startsWith(o.toLowerCase())),
      )
      .sort((a, b) => Number(!!a.last) - Number(!!b.last));

    for (const scenario of selected) {
      const transcript = new JsonlLog(
        path.join(dir, "scenarios", scenario.id, "transcript.jsonl"),
        secrets,
      );
      const sres = { checks: 0, failed: 0, notes: 0, error: null, results: {} };
      engineResults.scenarios[scenario.id] = sres;
      const ctx = {
        engine,
        persona: JSON.parse(
          await fsp.readFile(path.join(FIXTURES, "persona.json"), "utf8"),
        ),
        fixtureCopy: path.join(dir, "action", "fixtures"),
        marker,
        ledger,
        threads,
        logFile,
        settleMs: opts.settleMs,
        recallQualityChecked:
          engine === "builtin" ||
          !!engineResults.inference?.recall_quality_checked,
        cortex,
        composio,
        results: sres.results,
        get core() {
          return core;
        },
        rpc: (m, p, t) => core.rpc(m, p, t),
        tryRpc: (m, p, t) => core.tryRpc(m, p, t),
        markMeta: (extra = {}) => ({ tags: [marker], ...extra }),
        newThread(label) {
          const id = `thread-${randomUUID()}`;
          threads.push(id);
          transcript.write({ thread: id, label });
          return id;
        },
        async turn(threadId, message) {
          const r = await sendTurn({
            core,
            events,
            clientId: events.clientId,
            threadId,
            message,
          });
          await transcript.write({
            thread: threadId,
            user: message,
            reply: r.reply,
            error: r.error,
            ms: r.ms,
            tools: r.toolCalls,
          });
          return r;
        },
        async learn(text, kind = "fact", meta = {}) {
          const r = await core.rpc("openhuman.memory_learn", {
            text,
            kind,
            meta: { tags: [marker], ...meta },
          });
          ledger.add(r?.id);
          return r?.id;
        },
        async listAll(filter = {}, max = 2000) {
          const out = [];
          let cursor;
          do {
            const page = await core.rpc("openhuman.memory_items_list", {
              filter,
              limit: 100,
              ...(cursor ? { cursor } : {}),
            });
            out.push(...(page?.items ?? []));
            cursor = page?.next_cursor;
          } while (cursor && out.length < max);
          return out;
        },
        async waitItems(filter, atLeast, timeoutMs) {
          return waitFor(
            async () => {
              const items = await ctx.listAll(filter);
              return items.length >= atLeast ? items : null;
            },
            {
              timeoutMs,
              intervalMs: 1500,
              what: `${atLeast} item(s) for ${JSON.stringify(filter)}`,
            },
          ).catch(() => ctx.listAll(filter));
        },
        async setChatAgent(id) {
          await withRetries(
            async () => {
              await core.rpc("openhuman.config_update_agent_settings", {
                chat_agent_id: id,
              });
              const snap = await core.rpc("openhuman.config_get", {});
              if ((pick(snap, "config.agent.chat_agent_id") ?? "") !== id)
                throw new Error("chat_agent_id not applied");
            },
            { what: "chat_agent_id" },
          );
        },
        activeWorkspace: () => activeWorkspace(core),
        async setConfigToml(section, kv) {
          if (engine !== "local")
            throw new Error("config edits are local-engine only");
          const snap = await core.rpc("openhuman.config_get", {});
          await editToml(snap.config_path, section, kv);
        },
        async restartCore() {
          events?.close();
          await core.stop();
          await startCore();
        },
        check(id, ok, expected, actual, severity = "medium", basis = "READ") {
          sres.checks += 1;
          if (!ok) sres.failed += 1;
          return findings.check({
            engine,
            scenario: scenario.id,
            id,
            ok,
            expected,
            actual,
            evidence: path.relative(
              runDir,
              path.join(dir, "scenarios", scenario.id),
            ),
            severity,
            basis,
          });
        },
        note(id, text, basis = "READ") {
          sres.notes += 1;
          findings.note({
            engine,
            scenario: scenario.id,
            id,
            text,
            evidence: path.relative(
              runDir,
              path.join(dir, "scenarios", scenario.id),
            ),
            basis,
          });
        },
      };
      await fsp.cp(FIXTURES, ctx.fixtureCopy, { recursive: true });
      process.stdout.write(`${engine.padEnd(8)}: ${scenario.id} ... `);
      const started = Date.now();
      try {
        await scenario.run(ctx);
      } catch (e) {
        sres.error = scrub(e.stack || e.message, secrets);
        findings.add({
          engine,
          scenario: scenario.id,
          id: "crashed",
          severity: "high",
          expected: "the scenario runs to the end",
          actual: sres.error.slice(0, 600),
          evidence: "core.log",
          basis: "READ",
        });
      }
      sres.ms = Date.now() - started;
      console.log(
        `${sres.error ? "CRASHED" : "done"} ${sres.checks - sres.failed}/${sres.checks} checks, ${sres.notes} notes (${(sres.ms / 1000).toFixed(0)}s)`,
      );
    }
  } finally {
    // --- teardown -----------------------------------------------------------
    if (engine === "builtin" && core && !core.exited) {
      try {
        // Everything this run stored: the ledger, every item in its threads,
        // every item carrying its marker.
        for (const t of threads) {
          const items = await core
            .rpc("openhuman.memory_items_list", {
              filter: { thread_id: t },
              limit: 100,
            })
            .catch(() => null);
          ledger.addAll((items?.items ?? []).map((h) => h.id));
        }
        const tagged = await core
          .rpc("openhuman.memory_items_list", {
            filter: { tags_any: [marker] },
            limit: 100,
          })
          .catch(() => null);
        ledger.addAll((tagged?.items ?? []).map((h) => h.id));
        const survivors = await forgetLedger(core, ledger);
        const untouched = await verifyBuiltinUntouched(core);
        engineResults.cleanup = {
          stored: ledger.list().length,
          survivors,
          untouched,
        };
        if (survivors.length)
          findings.add({
            engine,
            scenario: "teardown",
            id: "survivors",
            severity: "high",
            expected: "every item this run stored is forgotten",
            actual: survivors,
            evidence: "rpc.jsonl",
            basis: "READ",
          });
        if (!untouched.ok)
          findings.add({
            engine,
            scenario: "teardown",
            id: "account-touched",
            severity: "high",
            expected: "no migration or import ran on the real account",
            actual: untouched,
            evidence: "rpc.jsonl",
            basis: "READ",
          });
        console.log(
          `builtin : cleanup forgot ${ledger.list().length} ids, ${survivors.length} survived; migration/import untouched: ${untouched.ok}`,
        );
      } catch (e) {
        findings.add({
          engine,
          scenario: "teardown",
          id: "cleanup-failed",
          severity: "high",
          expected: "cleanup completes",
          actual: scrub(e.message, secrets),
          evidence: "rpc.jsonl",
        });
      }
    }
    events?.close();
    await core?.stop();
    if (composio) {
      await fsp.writeFile(
        path.join(dir, "composio-requests.json"),
        JSON.stringify(composio.ctx.requests, null, 2),
      );
      await composio.close();
    }
    await cortex
      ?.stop()
      .catch((e) => console.error(`local   : teardown failed: ${e.message}`));
  }
}

// ---------------------------------------------------------------------------
// report
// ---------------------------------------------------------------------------

function renderReport({ runId, results, findings }) {
  const lines = [`# Memory scenarios ${runId}`, ""];
  for (const [engine, er] of Object.entries(results.engines)) {
    lines.push(`## ${engine}`, "");
    lines.push(`- inference: ${JSON.stringify(er.inference ?? {})}`);
    if (er.inference && er.inference.recall_quality_checked === false)
      lines.push(
        "- **recall-quality checks skipped**: Ollama was unreachable; the engine ran on the deterministic mock inference.",
      );
    if (er.cleanup) lines.push(`- cleanup: ${JSON.stringify(er.cleanup)}`);
    lines.push(
      "",
      "| scenario | checks passed | notes | time | crashed |",
      "| --- | --- | --- | --- | --- |",
    );
    for (const [id, s] of Object.entries(er.scenarios))
      lines.push(
        `| ${id} | ${s.checks - s.failed}/${s.checks} | ${s.notes} | ${((s.ms ?? 0) / 1000).toFixed(0)}s | ${s.error ? "yes" : ""} |`,
      );
    lines.push("");
  }
  const real = findings.filter((f) => f.severity !== "note");
  lines.push(
    `## Findings (${real.length})`,
    "",
    "| id | severity | basis | expected | actual |",
    "| --- | --- | --- | --- | --- |",
  );
  const order = { high: 0, medium: 1, low: 2 };
  for (const f of [...real].sort(
    (a, b) => (order[a.severity] ?? 3) - (order[b.severity] ?? 3),
  ))
    lines.push(
      `| ${f.id} | ${f.severity} | ${f.basis} | ${f.expected} | ${String(f.actual).replace(/\|/g, "\\|").slice(0, 300)} |`,
    );
  lines.push("", "## Notes", "");
  for (const f of findings.filter((x) => x.severity === "note"))
    lines.push(`- **${f.id}** (${f.basis}): ${f.actual}`);
  lines.push(
    "",
    "## Known gaps (not findings)",
    "",
    "- Channel messages are not logged to memory today, so the channel sender name (#7131) is inert: an open product question.",
  );
  return lines.join("\n") + "\n";
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  if (opts.gradeOnly) {
    const findings = JSON.parse(
      await fsp.readFile(path.join(opts.gradeOnly, "findings.json"), "utf8"),
    );
    const results = JSON.parse(
      await fsp.readFile(path.join(opts.gradeOnly, "results.json"), "utf8"),
    );
    const md = renderReport({
      runId: path.basename(opts.gradeOnly),
      results,
      findings,
    });
    await fsp.writeFile(path.join(opts.gradeOnly, "report.md"), md);
    console.log(md);
    return;
  }
  if (!fs.existsSync(opts.coreBin))
    throw new Error(
      `core binary not found at ${opts.coreBin}\nbuild it (debug, for the Composio override): cargo build -p openhuman-cli --bin openhuman-core`,
    );
  const engines = opts.engine === "both" ? ["local", "builtin"] : [opts.engine];
  if (engines.includes("builtin")) await builtinCredential(); // refuse early, before anything starts
  const runId = new Date().toISOString().replace(/[:.]/g, "-");
  const runDir = path.join(opts.runRoot, runId);
  await fsp.mkdir(runDir, { recursive: true });
  console.log(`run dir : ${runDir}`);

  const findings = new Findings();
  const results = {
    run_id: runId,
    started: new Date().toISOString(),
    engines: {},
  };
  for (const engine of engines) {
    try {
      await runEngine(engine, { opts, runDir, runId, findings, results });
    } catch (e) {
      const secretSafe = scrub(e.message, []);
      console.error(`${engine}: ${secretSafe}`);
      findings.add({
        engine,
        scenario: "setup",
        id: "aborted",
        severity: "high",
        expected: "the engine sets up",
        actual: secretSafe,
        evidence: `${engine}/core.log`,
      });
    }
  }
  results.finished = new Date().toISOString();
  await fsp.writeFile(
    path.join(runDir, "findings.json"),
    JSON.stringify(findings.items, null, 2),
  );
  await fsp.writeFile(
    path.join(runDir, "checks.json"),
    JSON.stringify(findings.checks, null, 2),
  );
  await fsp.writeFile(
    path.join(runDir, "results.json"),
    JSON.stringify(results, null, 2),
  );
  const md = renderReport({ runId, results, findings: findings.items });
  await fsp.writeFile(path.join(runDir, "report.md"), md);
  console.log(
    `\nreport  : ${path.join(runDir, "report.md")}\nfindings: ${findings.items.filter((f) => f.severity !== "note").length}`,
  );
}

main().catch((e) => {
  console.error(`\nfatal: ${scrub(e.stack || e.message, [])}`);
  process.exit(1);
});
