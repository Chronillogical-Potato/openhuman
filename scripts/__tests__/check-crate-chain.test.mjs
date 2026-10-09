import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  CHAIN,
  checkManifestEdges,
  checkRepository,
  findForbiddenPaths,
  formatReport,
  parseNormalDependencies,
  parsePackageName,
  parseWorkspaceDependencies,
  parseWorkspaceMembers,
} from '../ci/check-crate-chain.mjs';

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const CHECKER = resolve(REPO_ROOT, 'scripts/ci/check-crate-chain.mjs');

// ── parsing ────────────────────────────────────────────────────────────────

test('reads the package name and workspace members', () => {
  assert.equal(parsePackageName('[package]\nname = "openhuman-tui" # the TUI\n'), 'openhuman-tui');
  assert.deepEqual(
    parseWorkspaceMembers('[workspace]\nmembers = [\n  "crates/a",\n  # "crates/old",\n  "crates/b",\n]\n'),
    ['crates/a', 'crates/b']
  );
});

test('normal dependencies include target tables and sub-tables, not dev/build ones', () => {
  const deps = parseNormalDependencies(`
[package]
name = "openhuman-cli"

[dependencies]
openhuman-rpc = { workspace = true, features = ["server"] }
core-alias = { path = "../openhuman-core", package = "openhuman", features = [
    "voice",
] }
serde = "1"

[target.'cfg(unix)'.dependencies]
openhuman-embed = { path = "../openhuman-embed" }

[dependencies.openhuman-tinyhumans]
path = "../openhuman-tinyhumans"

[dev-dependencies]
openhuman-core = { path = "../openhuman-core", package = "openhuman" }

[build-dependencies]
openhuman-tui = { path = "../openhuman-tui" }
`);
  assert.deepEqual(
    deps.map(d => d.package),
    ['openhuman-rpc', 'openhuman', 'serde', 'openhuman-embed', 'openhuman-tinyhumans']
  );
  assert.equal(deps[0].workspace, true);
});

test('a commented-out dependency is not an edge', () => {
  const deps = parseNormalDependencies('[dependencies]\n# openhuman-core = { path = "x" }\nlog = "0.4"\n');
  assert.deepEqual(deps.map(d => d.key), ['log']);
});

test('workspace dependencies resolve their package rename', () => {
  const ws = parseWorkspaceDependencies(`
[workspace.dependencies]
openhuman-core = { path = "crates/openhuman-core", package = "openhuman" }
openhuman-rpc = { path = "crates/openhuman-rpc", default-features = false }
`);
  assert.equal(ws.get('openhuman-core'), 'openhuman');
  assert.equal(ws.get('openhuman-rpc'), 'openhuman-rpc');
});

// ── the chain ──────────────────────────────────────────────────────────────

test('each layer may name only the layer directly below it', () => {
  assert.deepEqual(CHAIN['openhuman-embed'], ['openhuman']);
  assert.deepEqual(CHAIN['openhuman-cli'], ['openhuman-rpc']);
  assert.deepEqual(checkManifestEdges({ crate: 'openhuman-tui', deps: [{ key: 'openhuman-rpc', package: 'openhuman-rpc', workspace: false }] }), []);
});

test('a host depending on the core is a violation, even through a workspace alias', () => {
  const workspaceDeps = new Map([['openhuman-core', 'openhuman']]);
  const violations = checkManifestEdges({
    crate: 'openhuman-tui',
    deps: [
      { key: 'openhuman-rpc', package: 'openhuman-rpc', workspace: true },
      { key: 'openhuman-core', package: 'openhuman-core', workspace: true },
      { key: 'openhuman-tinyhumans', package: 'openhuman-tinyhumans', workspace: false },
    ],
    workspaceDeps,
  });
  assert.deepEqual(
    violations.map(v => v.dependency),
    ['openhuman', 'openhuman-tinyhumans']
  );
});

test('the core may not depend on any OpenHuman crate', () => {
  const [violation] = checkManifestEdges({
    crate: 'openhuman',
    deps: [{ key: 'openhuman-rpc', package: 'openhuman-rpc', workspace: false }],
  });
  assert.match(violation.reason, /may not depend on any OpenHuman crate/);
});

test('an unknown OpenHuman crate fails until it is placed in the chain', () => {
  const [violation] = checkManifestEdges({ crate: 'openhuman-new', deps: [] });
  assert.match(violation.reason, /unknown OpenHuman crate/);
});

test('host source naming core internals by path is flagged, with file and line', () => {
  const hits = findForbiddenPaths(
    'use openhuman_rpc::embed::config;\nlet x = openhuman_core::config::Config::default();\nuse crate::core_host::agent;\nuse openhuman_embed::__host::core;\n',
    'src/lib.rs'
  );
  assert.deepEqual(
    hits.map(h => `${h.line}:${h.pattern}`),
    ['2:openhuman_core::', '3:core_host', '4:__host']
  );
});

test('a test name that merely contains openhuman_core is not a path', () => {
  assert.deepEqual(findForbiddenPaths('fn binary_path_result_contains_openhuman_core() {}', 'x.rs'), []);
});

// ── the real repository ────────────────────────────────────────────────────

test('the checked-in manifests and host sources hold the chain', () => {
  const result = checkRepository(REPO_ROOT);
  assert.ok(result.checked.length >= 7, `checked only ${result.checked.join(', ')}`);
  assert.ok(result.checked.includes('openhuman-app'), 'the desktop shell is outside the workspace and must still be checked');
  assert.deepEqual(result.edgeViolations, [], formatReport(result));
  assert.deepEqual(result.sourceViolations, [], formatReport(result));
});

test('the CLI exits 1 and names the edge when a host reaches past rpc', () => {
  const dir = mkdtempSync(join(tmpdir(), 'crate-chain-'));
  try {
    writeFileSync(
      join(dir, 'Cargo.toml'),
      '[workspace]\nmembers = ["crates/openhuman-core", "crates/openhuman-tui"]\n'
    );
    const crate = (name, toml) => {
      mkdirSync(join(dir, 'crates', name, 'src'), { recursive: true });
      writeFileSync(join(dir, 'crates', name, 'Cargo.toml'), toml);
    };
    crate('openhuman-core', '[package]\nname = "openhuman"\n');
    crate(
      'openhuman-tui',
      '[package]\nname = "openhuman-tui"\n[dependencies]\nopenhuman-core = { path = "../openhuman-core", package = "openhuman" }\n'
    );
    crate('openhuman-app', '[package]\nname = "openhuman-app"\n[dependencies]\nopenhuman-rpc = { path = "../openhuman-rpc" }\n');
    crate('openhuman-cli', '[package]\nname = "openhuman-cli"\n');
    writeFileSync(join(dir, 'crates/openhuman-cli/src/main.rs'), 'fn main() { openhuman_core::run(); }\n');
    const result = spawnSync('node', [CHECKER, dir], { encoding: 'utf8' });
    assert.equal(result.status, 1, result.stdout + result.stderr);
    assert.match(result.stderr, /openhuman-tui -> openhuman: may only depend on openhuman-rpc/);
    assert.match(result.stderr, /crates\/openhuman-cli\/src\/main\.rs:1: openhuman_core::/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('the CLI exits 2 instead of passing when it cannot read the repository', () => {
  const dir = mkdtempSync(join(tmpdir(), 'crate-chain-empty-'));
  try {
    const result = spawnSync('node', [CHECKER, dir], { encoding: 'utf8' });
    assert.equal(result.status, 2, result.stdout + result.stderr);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('the real repository passes through the CLI', () => {
  const result = spawnSync('node', [CHECKER], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /OK: every crate depends only on the layer below it/);
  // Unused import guard for cpSync in older Node lint configs.
  assert.equal(typeof cpSync, 'function');
});
