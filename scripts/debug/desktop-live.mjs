#!/usr/bin/env node
/**
 * Opt-in core-only desktop smoke test against an already-running local core.
 *
 * Prepare the disposable TextEdit document `desktop-e2e-noapproval.txt` first
 * for the textedit scenario. Use native tool dispatch for an OpenRouter model
 * that supports structured tool calls. Point --workspace at the running core's
 * actual workspace; a live signed-in profile works without changing its auth.
 * Supply the core bearer in OPENHUMAN_CORE_TOKEN and, if needed, a live
 * OPENROUTER_API_KEY. Then run:
 *
 *   node scripts/debug/desktop-live.mjs --live --workspace "$OPENHUMAN_WORKSPACE" \
 *     --model 'openai/gpt-4.1-mini' --rpc-url http://127.0.0.1:7788/rpc
 *
 * The script prints only method names and counts. It never prints a prompt,
 * transcript body, credential, accessibility value, or the random test marker.
 * The Calculator scenario makes one orchestrator turn and requires the goal
 * to complete multiple actions through one Jev call. The other scenarios make
 * bounded discovery, goal, and independent-readback turns.
 * Clean up the disposable document after the run.
 */

import { createHash, randomUUID } from 'node:crypto';
import { copyFile, mkdir, readFile, readdir, realpath, writeFile } from 'node:fs/promises';
import path from 'node:path';

function option(name, fallback) {
  const index = process.argv.indexOf(name);
  return index < 0 ? fallback : process.argv[index + 1];
}

function fail(message) {
  throw new Error(message);
}

const workspace = option('--workspace', process.env.OPENHUMAN_WORKSPACE);
const localModule = option('--prepare-local-module', null);
if (localModule) {
  if (!workspace) fail('Pass --workspace to prepare a local module.');
  const directory = path.join(workspace, 'desktop-live-module');
  const destination = path.join(directory, path.basename(localModule));
  await mkdir(directory, { recursive: true, mode: 0o700 });
  await copyFile(localModule, destination);
  const digest = createHash('sha256').update(await readFile(destination)).digest('hex');
  await writeFile(path.join(directory, 'modules.toml'),
    `${JSON.stringify(path.basename(destination))} = ${JSON.stringify(digest)}\n`,
    { mode: 0o600 });
  console.log('Local debug module copied and operator allowlist written.');
  const canonical = await realpath(destination);
  console.log(`Add [[modules.overrides]] with id = "tinydesktop" and path = ${JSON.stringify(canonical)} to the isolated core config, then restart the core.`);
  process.exit(0);
}
if (!process.argv.includes('--live')) fail('Pass --live to run real desktop actions.');
const rpcUrl = option('--rpc-url', 'http://127.0.0.1:7788/rpc');
const model = option('--model', process.env.OPENHUMAN_DESKTOP_TEST_MODEL);
const token = process.env.OPENHUMAN_CORE_TOKEN;
const openRouterKey = process.env.OPENROUTER_API_KEY;
const approveDisposable = process.argv.includes('--approve-disposable');
const localOfflineSession = process.argv.includes('--local-offline-session');
const scenario = option('--scenario', 'textedit');
if (!['textedit', 'spotify', 'spotify_pause', 'calculator'].includes(scenario)) {
  fail('--scenario must be textedit, spotify, spotify_pause, or calculator');
}
if (!workspace || !model || !token) {
  fail('Set --workspace, --model, and OPENHUMAN_CORE_TOKEN.');
}
if (!rpcUrl.startsWith('http://127.0.0.1:') && !rpcUrl.startsWith('http://localhost:')) {
  fail('The live driver accepts only a loopback core RPC URL.');
}

let sequence = 0;
async function rpc(method, params = {}) {
  const response = await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    body: JSON.stringify({ jsonrpc: '2.0', id: ++sequence, method, params }),
    signal: AbortSignal.timeout(240_000),
  });
  if (!response.ok) fail(`${method}: HTTP ${response.status}`);
  const body = await response.json();
  if (body.error) fail(`${method}: JSON-RPC error ${body.error.code ?? 'unknown'}`);
  return body.result;
}

async function transcriptFiles(directory) {
  const files = [];
  async function visit(dir) {
    for (const entry of await readdir(dir, { withFileTypes: true }).catch(() => [])) {
      const file = path.join(dir, entry.name);
      if (entry.isDirectory()) await visit(file);
      else if (entry.isFile() && file.endsWith('.jsonl')) files.push(file);
    }
  }
  await visit(directory);
  return files;
}

async function readThread(threadId) {
  const files = await transcriptFiles(path.join(workspace, 'session_raw'));
  const messages = [];
  for (const file of files) {
    const lines = (await readFile(file, 'utf8')).split(/\r?\n/).filter(Boolean);
    if (lines.length === 0) continue;
    let meta;
    try { meta = JSON.parse(lines[0])._meta; } catch { continue; }
    if (meta?.thread_id !== threadId || meta?.agent !== 'orchestrator') continue;
    for (const line of lines.slice(1)) {
      try { messages.push(JSON.parse(line)); } catch { /* partial append */ }
    }
  }
  return messages;
}

function calls(messages) {
  const names = [];
  for (const message of messages) {
    for (const call of message.tool_calls ?? []) {
      const name = call.function?.name ?? call.name;
      if (typeof name === 'string') names.push(name);
      if (name === 'tool_call') {
        let argumentsValue = call.function?.arguments ?? call.arguments;
        if (typeof argumentsValue === 'string') {
          try { argumentsValue = JSON.parse(argumentsValue); } catch { argumentsValue = {}; }
        }
        if (typeof argumentsValue?.name === 'string') names.push(argumentsValue.name);
      }
    }
  }
  return names;
}

function toolOutput(messages, wanted) {
  const targets = new Map();
  let result = null;
  for (const message of messages) {
    for (const call of message.tool_calls ?? []) {
      const name = call.function?.name ?? call.name;
      if (name !== 'tool_call') {
        targets.set(call.id, name);
        continue;
      }
      let args = call.function?.arguments ?? call.arguments;
      if (typeof args === 'string') {
        try { args = JSON.parse(args); } catch { args = {}; }
      }
      targets.set(call.id, args?.name);
    }
    if (message.role !== 'tool') continue;
    try {
      const wrapper = JSON.parse(message.content);
      if (targets.get(wrapper.tool_call_id) !== wanted) continue;
      result = typeof wrapper.content === 'string' ? JSON.parse(wrapper.content) : wrapper.content;
    } catch { /* Non-JSON tool output is not a structured result. */ }
  }
  return result;
}

const threadId = `desktop-live-${randomUUID()}`;
const marker = `OH desktop live ${randomUUID()}`;
const route = openRouterKey ? {
  inference_url: 'https://openrouter.ai/api/v1', api_key: openRouterKey,
} : {};

async function turn(label, message) {
  const before = (await readThread(threadId)).length;
  await rpc('openhuman.inference_agent_chat', {
    message, thread_id: threadId, model_override: model, ...route,
  });
  const transcript = await readThread(threadId);
  if (transcript.length <= before) fail(`${label}: no orchestrator transcript appeared`);
  console.log(`${label}: ${transcript.length - before} new transcript records`);
  return transcript;
}

if (localOfflineSession) {
  await rpc('openhuman.auth_set_credential', {
    token: `desktop-live.${randomUUID().replaceAll('-', '')}.local`,
    kind: 'local',
    user: { id: 'desktop-live', name: 'Desktop live test' },
  });
  console.log('auth: isolated offline local session installed');
}
let status = await rpc('openhuman.desktop_set_enabled', { enabled: true });
for (let attempt = 0; status.module_state === 'loading' && attempt < 15; attempt++) {
  await new Promise((resolve) => setTimeout(resolve, 2000));
  status = await rpc('openhuman.desktop_status');
}
if (!status.supported) fail('Desktop control is unsupported on this host.');
if (status.module_state === 'failed') fail('Desktop module failed to load; inspect local core logs.');
if (status.accessibility !== 'granted') fail('Grant Accessibility to the core process, then retry.');
console.log(`desktop: module=${status.module_state}, accessibility=${status.accessibility}, jev_ready=${status.jev_ready}, approvals_enabled=${status.approvals_enabled}`);
if (!status.jev_ready) fail('No Jev credential is available to the core.');
if (status.approvals_enabled !== false) fail('Live no-prompt run requires desktop approvals disabled.');

const appName = scenario === 'textedit' ? 'TextEdit' : scenario === 'calculator' ? 'Calculator' : 'Spotify';
if (scenario === 'calculator') {
  const transcript = await turn('single-turn',
    'In the native Calculator app, compute 12 + 34 and verify that the displayed result is 46. ' +
    'Use tool_search to discover desktop tools, launch Calculator, and inspect its accessibility snapshot. ' +
    'Then make exactly one desktop_goal call. Give it a bounded goal, the exact current window scope, ' +
    'CLICK as the only allowed mutation, exact target labels from the snapshot, and a value_equals or ' +
    'value_contains success predicate for the result display using its exact accessibility label. ' +
    'Allow up to 12 actions, 24 Jev decisions and 120 seconds. Do not use desktop_act. ' +
    'The Jev loop should do the calculator steps itself; do not make another desktop_goal call for each key. ' +
    'After the goal, read a fresh desktop_snapshot and report whether 46 is visible.');
  const observedCalls = calls(transcript);
  if (!observedCalls.includes('tool_search') || !observedCalls.includes('desktop_launch') ||
      !observedCalls.includes('desktop_goal') || !observedCalls.includes('desktop_snapshot')) {
    fail(`single-turn: missing desktop discovery, launch, goal or snapshot; saw ${observedCalls.join(', ')}`);
  }
  if (observedCalls.filter((name) => name === 'desktop_goal').length !== 1) {
    fail('single-turn: expected exactly one desktop_goal call');
  }
  const goal = toolOutput(transcript, 'desktop_goal');
  if (goal?.verified !== true || !Array.isArray(goal.turns) || goal.turns.length < 2) {
    fail(`single-turn: goal did not verify a multi-action result (stop=${goal?.stop ?? 'missing'}, steps=${goal?.turns?.length ?? 0})`);
  }
  const snapshot = toolOutput(transcript, 'desktop_snapshot');
  if (!JSON.stringify(snapshot ?? '').includes('46')) {
    fail('single-turn: independent Calculator snapshot did not contain result 46');
  }
  const pending = await rpc('openhuman.desktop_pending');
  const generic = await rpc('openhuman.approval_list_pending');
  const genericRows = Array.isArray(generic) ? generic : generic?.result;
  if ((Array.isArray(pending) && pending.length > 0) || !Array.isArray(genericRows) || genericRows.length > 0) {
    fail('single-turn: an approval remained pending');
  }
  console.log(`single-turn: one desktop_goal call, ${goal.turns.length} actions, verified result 46, no pending approvals`);
  console.log('PASS: Calculator task completed through one orchestrator turn and one Jev goal call.');
  process.exit(0);
}
let transcript = await turn('discover',
  `List the running desktop applications on this computer. Discover desktop tools with tool_search first, then call the matching desktop tool. Report only whether ${appName} is running.`);
let names = calls(transcript);
if (!names.includes('tool_search') || !names.includes('desktop_list_apps')) {
  fail(`discover: expected tool_search then desktop_list_apps; saw ${names.join(', ')}`);
}
console.log('discover: tool_search -> desktop_list_apps observed');

transcript = await turn('windows',
  `Use tool_search to find desktop_launch. Call desktop_launch with app ${appName} to activate its window, then call desktop_list_windows for ${appName}. Do not change app content.`);
names = calls(transcript);
if (!names.includes('desktop_launch')) fail(`windows: desktop_launch was not called; saw ${names.join(', ')}`);
if (!names.includes('desktop_list_windows')) fail(`windows: desktop_list_windows was not called; saw ${names.join(', ')}`);
console.log('windows: desktop_launch -> desktop_list_windows observed');
const listedWindows = toolOutput(transcript, 'desktop_list_windows');
const activeWindow = Array.isArray(listedWindows)
  ? listedWindows.find((window) => window.title === 'desktop-e2e-noapproval.txt' && window.accessible)
  : null;
if (scenario === 'textedit' && (!activeWindow?.title || !activeWindow?.id)) {
  fail('windows: disposable TextEdit test document is not available');
}

transcript = await turn('goal', scenario !== 'textedit'
  ? `In the running Spotify desktop app, ${scenario === 'spotify_pause' ? 'pause playback' : 'play the current track if paused'}. Use tool_search to find desktop_goal and run it with max_steps 4 and max_model_calls 8. Supply a name_present success predicate for the ${scenario === 'spotify_pause' ? 'Play' : 'Pause'} control using its exact accessibility name. Do not change playlists or account settings.`
  : `In the already open disposable TextEdit document, append this exact marker: ${marker}. ` +
    `The just-inspected disposable document is titled ${JSON.stringify(activeWindow.title)} and its editable field has the exact native AX identifier "First Text View". ` +
    `Use tool_search to find desktop_goal, then call it once with app TextEdit, window ${JSON.stringify(activeWindow.title)}, window_id ${JSON.stringify(activeWindow.id)}, ` +
    'allowed_operations [TYPE_TEXT], allowed_targets [First Text View], a text_slots entry keyed First Text View ' +
    'containing the marker, and a value_contains success predicate with name First Text View and value equal to the marker. ' +
    'Set max_steps 4 and max_model_calls 8. Let the goal tool make its own fresh observation. Do not save or close the document.');
names = calls(transcript);
if (!names.includes('desktop_goal')) fail(`goal: desktop_goal was not called; saw ${names.join(', ')}`);
const goalResult = toolOutput(transcript, 'desktop_goal');
if (!goalResult || !Array.isArray(goalResult.turns) || goalResult.turns.length < 1) {
  fail('goal: no desktop action was executed; an observed app state alone is insufficient');
}
if (goalResult.verified !== true) fail(`goal: result was not independently verified (stop=${goalResult.stop})`);
console.log(`goal: stop=${goalResult.stop}, executed_steps=${goalResult.turns.length}, jev_calls=${goalResult.metrics?.calls ?? 0}`);

const pending = await rpc('openhuman.desktop_pending');
const genericPending = await rpc('openhuman.approval_list_pending');
const genericRows = Array.isArray(genericPending) ? genericPending : genericPending?.result;
if (!Array.isArray(genericRows) || genericRows.length !== 0) {
  fail('An OpenHuman generic approval request was left pending during the desktop goal.');
}
if (Array.isArray(pending) && pending.length > 0) {
  console.log(`goal: ${pending.length} action(s) paused for user confirmation`);
  if (!approveDisposable) {
    fail('Inspect the pending action in Connections. Rerun with --approve-disposable only for a disposable TextEdit document.');
  }
  if (pending.length !== 1 || pending[0].app !== 'TextEdit') {
    fail('Approval refused: pending action is not the single TextEdit test action.');
  }
  await rpc('openhuman.desktop_confirm', {
    confirmation_id: pending[0].confirmation_id, approve: true,
  });
  transcript = await turn('continue',
    `Continue the previously paused TextEdit desktop goal using confirmation_id ${pending[0].confirmation_id}. ` +
    'The user approved the action in the trusted connection UI. Use tool_search to find desktop_continue_goal and call it with only that confirmation_id; the core restores the original goal.');
  if (!calls(transcript).includes('desktop_continue_goal')) fail('continue: desktop_continue_goal was not called');
  console.log('continue: trusted confirmation consumed by agent desktop_continue_goal call');
}

const beforeVerify = transcript.length;
transcript = await turn('verify', scenario !== 'textedit'
  ? `Read the full accessibility snapshot of the Spotify player using desktop_snapshot with skeleton false. Do not change playback. Report whether a ${scenario === 'spotify_pause' ? 'Play' : 'Pause'} control is visible.`
  : `Read the full accessibility snapshot of TextEdit window_id ${JSON.stringify(activeWindow.id)} using desktop_snapshot with skeleton false. Do not change the document.`);
const verifyRecords = transcript.slice(beforeVerify);
names = calls(verifyRecords);
if (!names.includes('desktop_snapshot')) fail(`verify: desktop_snapshot was not called; saw ${names.join(', ')}`);
const snapshot = toolOutput(verifyRecords, 'desktop_snapshot');
const snapshotText = JSON.stringify(snapshot ?? '');
const observed = scenario === 'textedit'
  ? snapshotText.includes(marker)
  : (scenario === 'spotify_pause' ? /\bPlay\b/ : /\bPause\b/).test(snapshotText);
if (!observed) fail(`verify: ${scenario === 'textedit' ? 'marker' : scenario === 'spotify_pause' ? 'Play control' : 'Pause control'} was not observed in a desktop tool result`);
console.log(`verify: ${scenario === 'textedit' ? 'marker' : scenario === 'spotify_pause' ? 'Play control' : 'Pause control'} observed in desktop_snapshot tool result`);
console.log(`PASS: direct-core orchestrator discovered and used desktop tools${scenario === 'textedit' ? '; clean up the disposable TextEdit document' : ''}.`);
