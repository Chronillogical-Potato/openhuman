#!/usr/bin/env node
/**
 * Life-scenario harness benchmark.
 *
 * Spawns an isolated `openhuman-core serve --jsonrpc-only`, seeds a sandbox
 * directory from `fixtures/`, drives one agent turn per scenario over
 * JSON-RPC, and reports tokens, prompt-cache hit rate, cost, latency and a
 * graded completion score.
 *
 * It measures the *core*, not the desktop app: no Tauri, no frontend, and a
 * `HOME` of its own so it never reads or writes the operator's `~/.openhuman`
 * — installing a credential there would sign a running desktop app out. Every
 * external dependency is pinned: inference goes to OpenRouter on the caller's
 * key, and Composio is a local mock. Nothing hosted is required.
 *
 * Usage:
 *   node scripts/life-scenarios/run.mjs                      # all scenarios
 *   node scripts/life-scenarios/run.mjs --only calendar-buffer,meal-plan
 *   node scripts/life-scenarios/run.mjs --model hint:agentic --repeat 2
 *   node scripts/life-scenarios/run.mjs --grade-only <run-dir>
 */

import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import fs from "node:fs";
import fsp from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { SCENARIOS, scenarioById } from "./scenarios.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, "..", "..");
const FIXTURES = path.join(HERE, "fixtures");

// ---------------------------------------------------------------------------
// args
// ---------------------------------------------------------------------------

function parseArgs(argv) {
  const o = {
    only: [],
    // Default route: OpenRouter, direct, via the per-turn ephemeral route.
    // The managed TinyHumans backend is deliberately NOT the default here —
    // a benchmark wants a route whose pricing and availability are its own,
    // not one that fails the whole run when the hosted provider is down.
    model: process.env.LIFE_SCENARIO_MODEL || "deepseek/deepseek-v4.1-flash",
    inferenceUrl:
      process.env.LIFE_SCENARIO_INFERENCE_URL || "https://openrouter.ai/api/v1",
    apiKey: process.env.OPENROUTER_API_KEY || "",
    managed: false,
    mockComposio: true,
    composioPort: 0,
    repeat: 1,
    turnTimeoutMs: 900_000,
    keep: false,
    coreBin: process.env.OPENHUMAN_CORE_BIN || path.join(REPO, "target", "debug", "openhuman-core"),
    runRoot: path.join(REPO, "target", "life-scenarios"),
    gradeOnly: "",
    verbose: false,
    approvalGate: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    const next = () => {
      const v = argv[++i];
      if (v === undefined) throw new Error(`missing value for ${a}`);
      return v;
    };
    if (a === "--only") o.only = next().split(",").map((s) => s.trim()).filter(Boolean);
    else if (a === "--model") o.model = next();
    else if (a === "--inference-url") o.inferenceUrl = next();
    else if (a === "--api-key") o.apiKey = next();
    else if (a === "--managed") o.managed = true;
    else if (a === "--no-mock-composio") o.mockComposio = false;
    else if (a === "--repeat") o.repeat = Number(next());
    else if (a === "--turn-timeout-ms") o.turnTimeoutMs = Number(next());
    else if (a === "--core-bin") o.coreBin = next();
    else if (a === "--run-root") o.runRoot = next();
    else if (a === "--grade-only") o.gradeOnly = next();
    else if (a === "--keep") o.keep = true;
    else if (a === "--approval-gate") o.approvalGate = true;
    else if (a === "--verbose" || a === "-v") o.verbose = true;
    else if (a === "-h" || a === "--help") {
      console.log(fs.readFileSync(path.join(HERE, "README.md"), "utf8"));
      process.exit(0);
    } else throw new Error(`unknown flag ${a}`);
  }
  return o;
}

// ---------------------------------------------------------------------------
// small utilities
// ---------------------------------------------------------------------------

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/**
 * The per-turn inference route.
 *
 * `inference_agent_chat` builds an `EphemeralRoute` only when BOTH
 * `inference_url` and `api_key` arrive non-blank, and `apply()` bails unless a
 * model is already resolved — so the model override is not optional on this
 * path, it is part of the route. `--managed` drops all three and lets the
 * turn run on the account's own configured provider instead.
 */
function routeParams(opts) {
  if (opts.managed) return opts.model ? { model_override: opts.model } : {};
  return {
    model_override: opts.model,
    inference_url: opts.inferenceUrl,
    api_key: opts.apiKey,
  };
}

/**
 * Mint the core's offline local session token.
 *
 * Custom (BYOK) providers sit behind `verify_session_active`
 * (crates/openhuman-core/src/inference/provider/factory/access_gates.rs), which
 * refuses a turn on a custom route unless the host installed a credential
 * first. A headless benchmark has no login flow, but the core has a documented
 * third credential kind for exactly this: a JWT-shaped token whose signature
 * segment is the literal `local` (`session_support::is_local_session_token`).
 *
 * This is the offline mode the core already ships, not a way around the gate:
 * it buys no access to the hosted backend, it only lets a local host say who
 * the turn belongs to. Every scenario run here reaches OpenRouter and a mock
 * Composio, so there is nothing hosted to authenticate against.
 */
function mintLocalSessionToken(userId) {
  const b64 = (o) =>
    Buffer.from(JSON.stringify(o))
      .toString("base64")
      .replace(/\+/g, "-")
      .replace(/\//g, "_")
      .replace(/=+$/, "");
  const now = Math.floor(Date.now() / 1000);
  const header = b64({ alg: "none", typ: "JWT" });
  const payload = b64({
    sub: userId,
    iat: now,
    exp: now + 24 * 60 * 60,
    email: "life-scenarios@local.invalid",
  });
  return `${header}.${payload}.local`;
}

async function freePort() {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.unref();
    srv.on("error", reject);
    srv.listen(0, "127.0.0.1", () => {
      const { port } = srv.address();
      srv.close(() => resolve(port));
    });
  });
}

async function copyDir(from, to) {
  await fsp.mkdir(to, { recursive: true });
  for (const entry of await fsp.readdir(from, { withFileTypes: true })) {
    const s = path.join(from, entry.name);
    const d = path.join(to, entry.name);
    if (entry.isDirectory()) await copyDir(s, d);
    else await fsp.copyFile(s, d);
  }
}

/** Walk a directory and return [{ rel, bytes, mtimeMs }] for every file. */
async function snapshotTree(root) {
  const out = [];
  async function walk(dir) {
    let entries;
    try {
      entries = await fsp.readdir(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const e of entries) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) await walk(p);
      else {
        const st = await fsp.stat(p).catch(() => null);
        if (st) out.push({ rel: path.relative(root, p), bytes: st.size, mtimeMs: st.mtimeMs });
      }
    }
  }
  await walk(root);
  return out;
}

// ---------------------------------------------------------------------------
// core lifecycle
// ---------------------------------------------------------------------------

class Core {
  constructor(opts) {
    this.opts = opts;
    this.proc = null;
    this.port = 0;
    this.token = randomBytes(24).toString("hex");
    this.logPath = "";
  }

  get url() {
    return `http://127.0.0.1:${this.port}`;
  }

  async start({ actionDir, logPath, approvalGate, home, composioBase }) {
    this.port = await freePort();
    this.logPath = logPath;
    const log = fs.createWriteStream(logPath, { flags: "a" });

    const env = {
      ...process.env,
      // A private HOME is the isolation boundary: `default_root_openhuman_dir`
      // resolves `<home>/.openhuman`, so config, keyring, auth profiles,
      // workspace and session db all land inside the run directory.
      HOME: home,
      OPENHUMAN_CORE_TOKEN: this.token,
      OPENHUMAN_CORE_PORT: String(this.port),
      OPENHUMAN_CORE_HOST: "127.0.0.1",
      // The agent's read/write root for every turn in this run.
      OPENHUMAN_ACTION_DIR: actionDir,
      // A headless benchmark has nobody to answer an approval prompt; without
      // this every tool call that the policy parks would sit until the ten
      // minute TTL expires it as DENIED. `--approval-gate` keeps it on so the
      // stall itself can be observed.
      ...(approvalGate ? {} : { OPENHUMAN_APPROVAL_GATE: "0" }),
      RUST_LOG: process.env.RUST_LOG || "info",
    };
    if (process.env.BACKEND_URL) env.BACKEND_URL = process.env.BACKEND_URL;
    if (composioBase) {
      // Both are read by `integrations/composio/client/factory.rs`; setting
      // them points the direct Composio client at the local mock.
      env.OPENHUMAN_COMPOSIO_DIRECT_BASE_V3 = composioBase;
      env.OPENHUMAN_COMPOSIO_DIRECT_BASE_V2 = composioBase;
    }

    this.proc = spawn(this.opts.coreBin, ["serve", "--jsonrpc-only"], {
      env,
      stdio: ["ignore", "pipe", "pipe"],
      cwd: actionDir,
    });
    this.proc.stdout.pipe(log);
    this.proc.stderr.pipe(log);
    this.proc.on("exit", (code, sig) => {
      this.exited = { code, sig };
    });

    const deadline = Date.now() + 120_000;
    while (Date.now() < deadline) {
      if (this.exited)
        throw new Error(
          `core exited during boot (code=${this.exited.code} sig=${this.exited.sig}); see ${logPath}`,
        );
      try {
        const r = await fetch(`${this.url}/health`, { signal: AbortSignal.timeout(3000) });
        if (r.ok) {
          const body = await r.json();
          return body;
        }
      } catch {
        /* not up yet */
      }
      await sleep(500);
    }
    throw new Error(`core did not become healthy on ${this.url}; see ${logPath}`);
  }

  async rpc(method, params, timeoutMs = 120_000) {
    const res = await fetch(`${this.url}/rpc`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${this.token}`,
      },
      body: JSON.stringify({ jsonrpc: "2.0", id: `ls-${Date.now()}`, method, params }),
      signal: AbortSignal.timeout(timeoutMs),
    });
    const text = await res.text();
    if (!res.ok) throw new Error(`RPC ${method} HTTP ${res.status}: ${text.slice(0, 400)}`);
    let body;
    try {
      body = JSON.parse(text);
    } catch {
      throw new Error(`RPC ${method} non-JSON: ${text.slice(0, 300)}`);
    }
    if (body.error)
      throw new Error(`RPC ${method} error: ${JSON.stringify(body.error).slice(0, 500)}`);
    // `apply_log_envelope` wraps a result that carried log lines.
    const r = body.result;
    if (r && typeof r === "object" && "result" in r && "logs" in r) return r.result;
    return r;
  }

  async stop() {
    if (!this.proc || this.exited) return;
    this.proc.kill("SIGTERM");
    const deadline = Date.now() + 10_000;
    while (!this.exited && Date.now() < deadline) await sleep(100);
    if (!this.exited) this.proc.kill("SIGKILL");
  }
}

// ---------------------------------------------------------------------------
// usage accounting
// ---------------------------------------------------------------------------

/**
 * `openhuman.cost_get_usage_log` returns one row per provider call. There is no
 * per-turn usage on the `inference_agent_chat` path, so a turn's usage is the
 * set of rows that appeared while it was running. Rows are keyed by `id`, so
 * diffing id sets is exact — a timestamp window would double-count a
 * concurrently running desktop core.
 */
async function usageIds(core) {
  const log = await core.rpc("openhuman.cost_get_usage_log", { days: 1, limit: 1000 });
  const records = (log && log.records) || [];
  return new Map(records.map((r) => [r.id, r]));
}

function foldUsage(rows) {
  const t = {
    calls: rows.length,
    input_tokens: 0,
    output_tokens: 0,
    cached_input_tokens: 0,
    cache_creation_tokens: 0,
    reasoning_tokens: 0,
    cost_usd: 0,
    models: new Set(),
    cost_sources: new Set(),
  };
  for (const r of rows) {
    t.input_tokens += Number(r.input_tokens || 0);
    t.output_tokens += Number(r.output_tokens || 0);
    t.cached_input_tokens += Number(r.cached_input_tokens || 0);
    t.cache_creation_tokens += Number(r.cache_creation_tokens || 0);
    t.reasoning_tokens += Number(r.reasoning_tokens || 0);
    t.cost_usd += Number(r.cost_usd || 0);
    if (r.model) t.models.add(r.model);
    if (r.cost_source) t.cost_sources.add(r.cost_source);
  }
  return {
    ...t,
    models: [...t.models],
    cost_sources: [...t.cost_sources],
    cache_hit_pct:
      t.input_tokens > 0 ? (t.cached_input_tokens / t.input_tokens) * 100 : 0,
  };
}

// ---------------------------------------------------------------------------
// grading
// ---------------------------------------------------------------------------

function gradeScenario(scenario, sandbox, transcript) {
  const ctx = {
    read(rel) {
      try {
        return fs.readFileSync(path.join(sandbox, rel), "utf8");
      } catch {
        return null;
      }
    },
    exists(rel) {
      return fs.existsSync(path.join(sandbox, rel));
    },
    transcript: transcript || "",
    sandbox,
  };
  let checks;
  try {
    ({ checks } = scenario.grade(ctx));
  } catch (e) {
    checks = [{ id: "grader_crashed", ok: false, detail: String(e.message).slice(0, 200) }];
  }
  const passed = checks.filter((c) => c.ok).length;
  return { checks, passed, total: checks.length, score: checks.length ? passed / checks.length : 0 };
}

// ---------------------------------------------------------------------------
// run one scenario
// ---------------------------------------------------------------------------

async function runScenario({ core, scenario, runDir, opts, attempt }) {
  const sandbox = path.join(runDir, "sandbox", scenario.id);
  await fsp.rm(sandbox, { recursive: true, force: true });
  await copyDir(FIXTURES, sandbox);
  await fsp.mkdir(path.join(sandbox, "out"), { recursive: true });
  const before = await snapshotTree(sandbox);

  const usageBefore = await usageIds(core);
  const threadId = `ls-${scenario.id}-${attempt}-${randomBytes(3).toString("hex")}`;

  const t0 = Date.now();
  let reply = null;
  let error = null;
  try {
    reply = await core.rpc(
      "openhuman.inference_agent_chat",
      {
        message: scenario.prompt,
        thread_id: threadId,
        cwd: sandbox,
        ...routeParams(opts),
      },
      opts.turnTimeoutMs,
    );
  } catch (e) {
    error = String(e.message).slice(0, 600);
  }
  const latencyMs = Date.now() - t0;

  // Cost rows can land a beat after the RPC returns.
  await sleep(1500);
  const usageAfter = await usageIds(core);
  const newRows = [...usageAfter.entries()]
    .filter(([id]) => !usageBefore.has(id))
    .map(([, r]) => r);
  const usage = foldUsage(newRows);

  const after = await snapshotTree(sandbox);
  const beforeSet = new Map(before.map((f) => [f.rel, f]));
  const written = after.filter(
    (f) => !beforeSet.has(f.rel) || beforeSet.get(f.rel).mtimeMs !== f.mtimeMs,
  );

  const grade = gradeScenario(scenario, sandbox, typeof reply === "string" ? reply : "");

  return {
    scenario: scenario.id,
    title: scenario.title,
    attempt,
    thread_id: threadId,
    ok: !error,
    error,
    latency_ms: latencyMs,
    reply_chars: typeof reply === "string" ? reply.length : 0,
    files_written: written.map((f) => ({ rel: f.rel, bytes: f.bytes })),
    usage,
    grade,
    sandbox,
  };
}

// ---------------------------------------------------------------------------
// reporting
// ---------------------------------------------------------------------------

const fmtUsd = (n) => `$${n.toFixed(4)}`;
const fmtTok = (n) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n));

function printReport(results) {
  const rows = results.map((r) => ({
    scenario: r.scenario,
    completion: `${r.grade.passed}/${r.grade.total}`,
    pct: `${Math.round(r.grade.score * 100)}%`,
    calls: r.usage.calls,
    in: fmtTok(r.usage.input_tokens),
    cached: fmtTok(r.usage.cached_input_tokens),
    "cache%": `${r.usage.cache_hit_pct.toFixed(1)}%`,
    out: fmtTok(r.usage.output_tokens),
    cost: fmtUsd(r.usage.cost_usd),
    latency: `${(r.latency_ms / 1000).toFixed(1)}s`,
    status: r.ok ? "ok" : "ERROR",
  }));
  console.log("");
  console.table(rows);

  const tot = results.reduce(
    (a, r) => {
      a.calls += r.usage.calls;
      a.input += r.usage.input_tokens;
      a.cached += r.usage.cached_input_tokens;
      a.output += r.usage.output_tokens;
      a.cost += r.usage.cost_usd;
      a.latency += r.latency_ms;
      a.passed += r.grade.passed;
      a.total += r.grade.total;
      return a;
    },
    { calls: 0, input: 0, cached: 0, output: 0, cost: 0, latency: 0, passed: 0, total: 0 },
  );
  console.log(
    `TOTAL  completion ${tot.passed}/${tot.total} (${Math.round(
      (tot.passed / Math.max(1, tot.total)) * 100,
    )}%)  calls ${tot.calls}  in ${fmtTok(tot.input)}  cached ${fmtTok(
      tot.cached,
    )} (${((tot.cached / Math.max(1, tot.input)) * 100).toFixed(1)}%)  out ${fmtTok(
      tot.output,
    )}  cost ${fmtUsd(tot.cost)}  wall ${(tot.latency / 1000).toFixed(1)}s`,
  );

  for (const r of results) {
    const failed = r.grade.checks.filter((c) => !c.ok);
    if (!failed.length && r.ok) continue;
    console.log(`\n  ${r.scenario}:`);
    if (r.error) console.log(`    ! turn error: ${r.error}`);
    for (const f of failed) console.log(`    x ${f.id}${f.detail ? ` — ${f.detail}` : ""}`);
  }
  console.log("");
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

async function main() {
  const opts = parseArgs(process.argv.slice(2));

  if (opts.gradeOnly) {
    const results = [];
    for (const s of SCENARIOS) {
      const sandbox = path.join(opts.gradeOnly, "sandbox", s.id);
      if (!fs.existsSync(sandbox)) continue;
      results.push({
        scenario: s.id,
        title: s.title,
        ok: true,
        error: null,
        latency_ms: 0,
        usage: foldUsage([]),
        grade: gradeScenario(s, sandbox, ""),
      });
    }
    printReport(results);
    return;
  }

  const selected = opts.only.length ? opts.only.map(scenarioById) : SCENARIOS;
  const runId = new Date().toISOString().replace(/[:.]/g, "-");
  const runDir = path.join(opts.runRoot, runId);
  await fsp.mkdir(runDir, { recursive: true });

  // One shared action_dir parent so a single core covers every scenario; each
  // scenario still gets its own subdirectory, passed per turn as `cwd`.
  const actionRoot = path.join(runDir, "sandbox");
  await fsp.mkdir(actionRoot, { recursive: true });

  const core = new Core(opts);
  console.log(`run dir : ${runDir}`);
  console.log(`core bin: ${opts.coreBin}`);
  if (!fs.existsSync(opts.coreBin))
    throw new Error(
      `core binary not found at ${opts.coreBin}\n` +
        `build it: cargo build --manifest-path Cargo.toml -p openhuman-cli --bin openhuman-core`,
    );

  const health = await core.start({
    actionDir: actionRoot,
    logPath: path.join(runDir, "core.log"),
    approvalGate: opts.approvalGate,
  });
  console.log(`core    : ${core.url} (pid ${health.pid}, healthy=${health.healthy})`);

  // Install the offline local credential before the first turn.
  const localUserId = `life-scenarios-${runId.slice(0, 10).replace(/-/g, "")}`;
  await core.rpc("openhuman.auth_set_credential", {
    token: mintLocalSessionToken(localUserId),
    kind: "local",
    userId: localUserId,
    user: { _id: localUserId, email: "life-scenarios@local.invalid", name: "Life Scenarios" },
  });
  const authState = await core.rpc("openhuman.auth_get_state", {}).catch(() => null);
  console.log(
    `auth    : local session installed (${
      authState ? JSON.stringify(authState).slice(0, 120) : "state unavailable"
    })`,
  );
  console.log(
    `route   : ${opts.managed ? "managed backend" : opts.inferenceUrl} model=${opts.model}`,
  );

  const results = [];
  try {
    // A cheap smoke turn proves the credential and route work before we spend
    // real money on six long scenarios, and warms the prompt cache.
    const smokeStart = Date.now();
    const smoke = await core.rpc(
      "openhuman.inference_agent_chat",
      {
        message: "Reply with exactly: READY",
        thread_id: `ls-smoke-${runId}`,
        ...routeParams(opts),
      },
      120_000,
    );
    console.log(
      `smoke   : ${(Date.now() - smokeStart) / 1000}s -> ${String(smoke).slice(0, 80)}`,
    );

    for (let attempt = 1; attempt <= opts.repeat; attempt += 1) {
      for (const scenario of selected) {
        process.stdout.write(`running ${scenario.id} (attempt ${attempt}) ... `);
        const r = await runScenario({ core, scenario, runDir, opts, attempt });
        results.push(r);
        console.log(
          `${r.ok ? "ok" : "ERROR"} ${(r.latency_ms / 1000).toFixed(1)}s ` +
            `${r.grade.passed}/${r.grade.total} ${fmtUsd(r.usage.cost_usd)}`,
        );
        await fsp.writeFile(
          path.join(runDir, "results.json"),
          JSON.stringify(results, null, 2),
        );
      }
    }
  } finally {
    await core.stop();
  }

  printReport(results);
  console.log(`results : ${path.join(runDir, "results.json")}`);
  console.log(`core log: ${path.join(runDir, "core.log")}`);
}

main().catch((e) => {
  console.error(`\nfatal: ${e.stack || e.message}`);
  process.exit(1);
});
