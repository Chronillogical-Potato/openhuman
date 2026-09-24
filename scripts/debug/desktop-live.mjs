#!/usr/bin/env node
/**
 * Opt-in core-only desktop smoke test against an already-running local core.
 *
 * Prepare a disposable TextEdit document first for the textedit scenario. Set
 * `[agent] tool_dispatcher = "native"` in the isolated config for an OpenRouter
 * model that supports structured tool calls. Start the core with an
 * isolated OPENHUMAN_WORKSPACE, an operator-supplied OPENHUMAN_CORE_TOKEN (so it
 * writes no core.token file), and OPENROUTER_API_KEY when no TinyHumans session
 * is present. Then run:
 *
 *   node scripts/debug/desktop-live.mjs --live --workspace "$OPENHUMAN_WORKSPACE" \
 *     --model 'openai/gpt-4.1-mini' --rpc-url http://127.0.0.1:7788/rpc
 *
 * The script prints only method names and counts. It never prints a prompt,
 * transcript body, credential, accessibility value, or the random test marker.
 * It makes three bounded orchestrator turns: discover/list apps, type into the
 * disposable document through Jev, and read the value back through accessibility.
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
if (!['textedit', 'spotify', 'spotify_pause'].includes(scenario)) fail('--scenario must be textedit, spotify, or spotify_pause');
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

const appName = scenario === 'textedit' ? 'TextEdit' : 'Spotify';
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

transcript = await turn('goal', scenario !== 'textedit'
  ? `In the running Spotify desktop app, ${scenario === 'spotify_pause' ? 'pause playback' : 'play the current track if paused'}. Use tool_search to find desktop_goal and run it with max_steps 4 and max_model_calls 8. Verify the player shows a ${scenario === 'spotify_pause' ? 'Play' : 'Pause'} control. Do not change playlists or account settings.`
  : `In the already open disposable TextEdit document, append this exact marker: ${marker}. ` +
    'Use tool_search to find desktop_goal. Pass the marker as the first value in its text array, and run the bounded goal with max_steps 4 and max_model_calls 8. Do not save or close the document.');
names = calls(transcript);
if (!names.includes('desktop_goal')) fail(`goal: desktop_goal was not called; saw ${names.join(', ')}`);
const goalResult = toolOutput(transcript, 'desktop_goal');
if (!goalResult || !Array.isArray(goalResult.turns) || goalResult.turns.length < 1) {
  fail('goal: no desktop action was executed; an observed app state alone is insufficient');
}
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
  : 'Read the full accessibility snapshot of the current TextEdit document using desktop_snapshot with skeleton false. Do not change the document.');
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
