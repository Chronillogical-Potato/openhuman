// The two engines a run tests.
//
// - local: a throwaway CortexDB container per run (its own compose project,
//   volume and port; never the user's `cortexdb` / `cortexdb-data` / :3141),
//   inference through the local Ollama when reachable.
// - builtin: the user's real account. Guards in `builtinGuards` keep the run
//   from moving, erasing or importing anything of the account's own.

import fsp from "node:fs/promises";
import path from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { randomBytes } from "node:crypto";
import { freePort, waitFor, pick } from "./lib.mjs";

const exec = promisify(execFile);

/** Names a run must never touch. */
const FORBIDDEN_PROJECTS = new Set([
  "cortexdb",
  "tinymemory-cortexdb",
  "tinymemory-cortexdb-test",
]);
const FORBIDDEN_PORT = 3141;

const OLLAMA = process.env.MEMSCEN_OLLAMA_URL || "http://127.0.0.1:11434";
export const OLLAMA_EMBED_MODEL =
  process.env.MEMSCEN_EMBED_MODEL || "nomic-embed-text";
export const OLLAMA_EMBED_DIMS = process.env.MEMSCEN_EMBED_DIMS || "768";
export const OLLAMA_CHAT_MODEL =
  process.env.MEMSCEN_CHAT_MODEL || "llama3.2:3b";

/** Ollama's model names, or null when it is unreachable. */
async function ollamaModels() {
  try {
    const r = await fetch(`${OLLAMA}/api/tags`, {
      signal: AbortSignal.timeout(5000),
    });
    if (!r.ok) return null;
    return ((await r.json()).models ?? []).map((m) => m.name);
  } catch {
    return null;
  }
}

/** Pull `model` through Ollama's API when it is missing. */
async function ensureOllamaModel(model, have) {
  const want = model.includes(":") ? model : `${model}:latest`;
  if (have.includes(want) || have.includes(model)) return;
  const r = await fetch(`${OLLAMA}/api/pull`, {
    method: "POST",
    body: JSON.stringify({ model, stream: false }),
    signal: AbortSignal.timeout(20 * 60_000),
  });
  if (!r.ok) throw new Error(`ollama pull ${model}: HTTP ${r.status}`);
}

/**
 * Start the per-run CortexDB. Returns `{endpoint, apiKey, inference, stop}`.
 * `inference` says what backs embeddings and extraction, for report.md.
 */
export async function startLocalCortex({
  runId,
  scriptDir,
  tinymemoryDir,
  keep,
  log,
}) {
  const project = `memscen-${runId
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, "")
    .slice(-24)}`;
  if (FORBIDDEN_PROJECTS.has(project))
    throw new Error(`refusing compose project ${project}`);
  const port = await freePort();
  if (port === FORBIDDEN_PORT) throw new Error("refusing port 3141");
  const apiKey = `memscen-${randomBytes(12).toString("hex")}`;

  const models = await ollamaModels();
  let compose;
  let env;
  let inference;
  if (models) {
    await ensureOllamaModel(OLLAMA_EMBED_MODEL, models);
    await ensureOllamaModel(OLLAMA_CHAT_MODEL, models);
    compose = ["-f", path.join(scriptDir, "cortexdb", "docker-compose.yml")];
    env = {
      CORTEXDB_VERSION: "v0.10.5",
      MEMSCEN_CORTEX_KEY: apiKey,
      MEMSCEN_CORTEX_PORT: String(port),
      // From inside the container the host's loopback is host.docker.internal.
      CORTEX_INFERENCE_URL: `${OLLAMA.replace("127.0.0.1", "host.docker.internal").replace("localhost", "host.docker.internal")}/v1`,
      CORTEX_EMBEDDING_MODEL: OLLAMA_EMBED_MODEL,
      CORTEX_EMBEDDING_DIMS: OLLAMA_EMBED_DIMS,
      CORTEX_CHAT_MODEL: OLLAMA_CHAT_MODEL,
    };
    inference = {
      kind: "ollama",
      embeddings: `${OLLAMA_EMBED_MODEL} (${OLLAMA_EMBED_DIMS} dims)`,
      chat: OLLAMA_CHAT_MODEL,
      recall_quality_checked: true,
    };
  } else {
    // tinymemory's compose with its deterministic inference double: plumbing
    // still works, retrieval is not semantic.
    compose = [
      "-f",
      path.join(tinymemoryDir, "integration", "cortexdb", "docker-compose.yml"),
    ];
    env = {
      CORTEXDB_VERSION: "v0.10.5",
      CORTEXDB_PORT: String(port),
      TINYMEMORY_TEST_CORTEX_KEY: apiKey,
    };
    inference = {
      kind: "mock-inference",
      embeddings: "deterministic",
      chat: "deterministic",
      recall_quality_checked: false,
    };
  }

  const dc = (...args) =>
    exec(
      "docker",
      ["compose", "--project-name", project, ...compose, ...args],
      {
        env: { ...process.env, ...env },
        maxBuffer: 16 * 1024 * 1024,
      },
    );
  log?.(
    `local   : compose project ${project}, port ${port}, inference ${inference.kind}`,
  );
  await dc(
    "up",
    "-d",
    ...(models ? [] : ["--build", "--wait", "mock-inference"]),
  );
  if (!models) await dc("up", "-d", "cortex");
  const endpoint = `http://127.0.0.1:${port}`;
  await waitFor(
    async () =>
      (
        await fetch(`${endpoint}/v1/admin/ready`, {
          signal: AbortSignal.timeout(3000),
        })
      ).ok,
    { timeoutMs: 180_000, intervalMs: 1000, what: "CortexDB ready" },
  );
  const version = await fetch(`${endpoint}/v1/admin/version`)
    .then((r) => r.text())
    .catch(() => "?");

  return {
    project,
    endpoint,
    apiKey,
    inference: { ...inference, cortexdb_version: version.trim() },
    /** Stop the CortexDB container (to force engine errors), and start it again. */
    pause: () => dc("stop", "cortex"),
    resume: async () => {
      await dc("start", "cortex");
      await waitFor(
        async () => (await fetch(`${endpoint}/v1/admin/ready`)).ok,
        {
          timeoutMs: 120_000,
          what: "CortexDB ready again",
        },
      );
    },
    async stop() {
      await dc("down", "--remove-orphans", ...(keep ? [] : ["--volumes"]));
    },
  };
}

/**
 * Read CortexDB directly with the run's own key, for checks that need what the
 * engine stored rather than what the core reports (labels, basenames, scopes).
 */
export function cortexReader({ endpoint, apiKey }) {
  const call = async (method, route, body) => {
    const r = await fetch(`${endpoint}${route}`, {
      method,
      headers: {
        authorization: `Bearer ${apiKey}`,
        "content-type": "application/json",
      },
      body: body ? JSON.stringify(body) : undefined,
      signal: AbortSignal.timeout(30_000),
    });
    const text = await r.text();
    let json;
    try {
      json = JSON.parse(text);
    } catch {
      json = { raw: text };
    }
    return { status: r.status, body: json };
  };
  return {
    get: (route) => call("GET", route),
    post: (route, body) => call("POST", route, body),
  };
}

// ---------------------------------------------------------------------------
// builtin: the real account
// ---------------------------------------------------------------------------

export const API_KEY_ENV = "OPENHUMAN_BACKEND_API_KEY";
const KEY_FILE =
  process.env.MEMSCEN_KEY_FILE ||
  path.join(process.env.HOME || "", ".memscen-key");

/**
 * The account's TinyHumans API key, read from ~/.memscen-key at spawn time.
 * It reaches the core only as OPENHUMAN_BACKEND_API_KEY (seeded at boot by
 * security/credentials/ops/boot_env.rs); it is never printed, logged, written
 * to the run dir or passed to an RPC. The run refuses builtin without it.
 */
export async function builtinCredential() {
  let key;
  try {
    key = (await fsp.readFile(KEY_FILE, "utf8")).trim();
  } catch {
    throw new Error(`builtin needs a TinyHumans API key in ${KEY_FILE}`);
  }
  if (!/^tiny_[A-Za-z0-9_-]{8,}$/.test(key))
    throw new Error(`${KEY_FILE} does not hold a tiny_ API key`);
  return { env: { [API_KEY_ENV]: key }, secret: key };
}

/** The active workspace directory, read from the running core. */
export async function activeWorkspace(core) {
  const snap = await core.rpc("openhuman.config_get", {});
  const cfg = pick(snap, "config", "snapshot.config", "snapshot") ?? snap;
  const ws = pick(cfg, "workspace_dir") ?? pick(snap, "workspace_dir");
  if (!ws) throw new Error("config_get carries no workspace_dir");
  return { workspace: ws, config: cfg };
}

/**
 * Guard 1: mark the layout migration done for this throwaway workspace, so the
 * memory background job's tick never starts moving the account's real legacy
 * tree. Written as the job's own state file; `migration_status` then reads
 * `cleaned`. Returns the state read back, or throws.
 */
export async function placeMigrationGuard(core, statePath) {
  await fsp.mkdir(path.dirname(statePath), { recursive: true });
  const state = {
    phase: "cleaned",
    copied: 0,
    replayed: 0,
    switched: true,
    caught_up: true,
    cleaning: false,
    takeover: false,
  };
  await fsp.writeFile(statePath, JSON.stringify(state, null, 2));
  const status = await core.rpc("openhuman.memory_migration_status", {});
  const phase = pick(status, "state.phase");
  if (phase !== "cleaned" || status.running)
    throw new Error(
      `migration guard not in effect: phase=${phase} running=${status.running}`,
    );
  return status;
}

/** Guard 1, checked at the end: nothing moved, nothing imported. */
export async function verifyBuiltinUntouched(core) {
  const migration = await core.rpc("openhuman.memory_migration_status", {});
  const imp = await core.rpc("openhuman.memory_import_status", {});
  return {
    migration_phase: pick(migration, "state.phase"),
    migration_running: migration.running,
    migration_copied: pick(migration, "state.copied"),
    import_phase: pick(imp, "state.phase"),
    ok:
      pick(migration, "state.phase") === "cleaned" &&
      !migration.running &&
      (pick(migration, "state.copied") ?? 0) === 0 &&
      ["idle", undefined].includes(pick(imp, "state.phase")),
  };
}

/**
 * Guard 2: forget exactly the ids this run stored, then confirm each is gone.
 * Returns the ids that survived.
 */
export async function forgetLedger(core, ledger) {
  const ids = ledger.list();
  for (let i = 0; i < ids.length; i += 50)
    await core.tryRpc("openhuman.memory_forget", { ids: ids.slice(i, i + 50) });
  const survivors = [];
  for (let i = 0; i < ids.length; i += 50) {
    const got = await core.tryRpc("openhuman.memory_items_get", {
      ids: ids.slice(i, i + 50),
    });
    for (const item of got.ok ? (got.value?.items ?? []) : [])
      survivors.push(item.id);
  }
  return survivors;
}
