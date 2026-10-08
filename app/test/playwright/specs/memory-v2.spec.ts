import { expect, type Page, type Request, type Route, test } from '@playwright/test';

import {
  bootAuthenticatedPage,
  bootRuntimeReadyGuestPage,
  callCoreRpc,
  dismissWalkthroughIfPresent,
  waitForAppReady,
} from '../helpers/core-rpc';

/**
 * Memory v2 page (`/connections?tab=brain`, docs/specs/memory-v2.md).
 *
 * Drives the real page in a browser through its three chips — Provider
 * (engine), Conversations and Files (`brain`) — plus the "import previous
 * memory" consent flow (at the top of Files) and the memory-off state. The
 * retired chips (ask, explorer, learnings, background, settings, migration)
 * must redirect to a kept chip.
 *
 * WHY THE MEMORY RPC IS MOCKED: the page only talks to the core through the
 * `openhuman.memory_*` v2 methods (services/api/memoryApi.ts), and the engines
 * behind them are hosted (TinyHumans) or self-hosted (CortexDB) services this
 * lane must not reach. The v2 controllers may also not be in the core build the
 * lane runs against yet. So every v2 memory method is answered in-browser by a
 * small stateful fake (sources grow on add, an import goes idle -> running ->
 * done), installed with `page.route` BEFORE the page
 * is opened. Every other method — auth, config, the app shell — passes through
 * to the real core with `route.continue()`, exactly as
 * `token-usage-load-failure.spec.ts` does, so only the memory surface is faked.
 */

const MEMORY_URL = '/#/connections?tab=brain';

const ENGINES = [
  {
    id: 'tinyhumans',
    label: 'TinyHumans',
    description: 'Hosted memory for your account.',
    hosted: true,
    needs_endpoint: false,
    needs_key: false,
    default_endpoint: null,
    fetch_modes: ['keyword', 'vector', 'hybrid'],
  },
  {
    id: 'cortexdb',
    label: 'CortexDB',
    description: 'Your own CortexDB instance.',
    hosted: false,
    needs_endpoint: true,
    needs_key: true,
    default_endpoint: 'https://api-v1.cortexdb.ai',
    fetch_modes: ['keyword', 'vector'],
  },
];

interface FakeOptions {
  /** `memory_engine_get` reports TinyHumans as active (`ok`) when true, off when false. */
  engineOn: boolean;
  /** `memory_import_scan` finds v1 data to import. */
  importFound?: boolean;
  /** An earlier import stopped with this error (e.g. credits ran out) after 7 of 20 items. */
  importStoppedWith?: string;
  /** An earlier import finished with this many items the engine refused. */
  importFinishedWithFailed?: number;
  /** `memory_migration_scan` finds legacy memory to move; `shared` when other accounts may share it. */
  migration?: { shared: boolean };
  /**
   * Methods (without `openhuman.`) answered with a JSON-RPC error instead,
   * shaped like the core's memory error (`data.code` / `data.kind`).
   */
  failWith?: Record<string, RpcFailure>;
}

interface RpcFailure {
  code: string;
  message: string;
}

/** The core's refusal for an exhausted credit balance (memory/error.rs). */
const OUT_OF_CREDITS: RpcFailure = {
  code: 'INSUFFICIENT_CREDITS',
  message:
    'insufficient credits: [USER_INSUFFICIENT_CREDITS] memory API recall: the account has insufficient credits (HTTP 402)',
};

interface RpcCall {
  method: string;
  params: Record<string, unknown>;
}

interface MemoryFake {
  /** Every intercepted memory call, in order, with the params the page sent. */
  calls: RpcCall[];
  /** Params of each call to `method` (without the `openhuman.` prefix). */
  paramsOf(method: string): Record<string, unknown>[];
}

/**
 * Install the in-browser fake for the v2 memory RPC surface. Must run before
 * the Memory page is opened so its first `memory_engine_get` is answered here.
 */
async function installMemoryFake(page: Page, opts: FakeOptions): Promise<MemoryFake> {
  const calls: RpcCall[] = [];

  const engineState = () =>
    opts.engineOn
      ? { engine: 'tinyhumans', has_key: false, status: 'ok', fetch_modes: ['keyword', 'vector'] }
      : { engine: null, has_key: false, status: 'off', fetch_modes: [] };

  const sources: Array<Record<string, unknown>> = [];
  let nextId = 1;
  let migrating = false;
  let migrated = false;
  let importState = opts.importStoppedWith
    ? {
        phase: 'error',
        imported: 7,
        total: 20,
        error: opts.importStoppedWith as string | null,
        failed: 0,
      }
    : opts.importFinishedWithFailed !== undefined
      ? {
          phase: 'done',
          imported: 20 - opts.importFinishedWithFailed,
          total: 20,
          error: null as string | null,
          failed: opts.importFinishedWithFailed,
        }
      : { phase: 'idle', imported: 0, total: 0, error: null as string | null, failed: 0 };

  const policy = {
    log_conversations: true,
    recall: {
      enabled: true,
      budget_tokens: 1500,
      learnings_limit: 8,
      brain_limit: 6,
      history_limit: 6,
      team_limit: 4,
      build_beliefs_every: 20,
      pre_turn_timeout_ms: 1500,
      compaction_timeout_ms: 5000,
      build_delay_secs: 30,
    },
    root: 'user:e2e',
    agent_id: 'main',
    host_bound: false,
  };
  /** The fake's answer per v2 method; `undefined` = not a v2 method, pass through. */
  const handle = (method: string, params: Record<string, unknown>): unknown => {
    switch (method) {
      case 'memory_engines_list':
        return { engines: ENGINES, active: opts.engineOn ? 'tinyhumans' : null };
      case 'memory_engine_get':
        return engineState();
      case 'memory_engine_set':
        // Echo what was set, so the tab marks the option it maps to as active
        // (a loopback endpoint is Self-host, none is the API-key option).
        return {
          engine: params.engine,
          ...(params.endpoint ? { endpoint: params.endpoint } : {}),
          has_key: Boolean(params.api_key),
          status: 'ok',
          fetch_modes: ['keyword', 'vector'],
        };
      case 'memory_fetch':
        return { hits: [], next_cursor: null };
      case 'memory_items_list':
        return { items: [], next_cursor: null };
      case 'memory_policy_get':
        return policy;
      case 'memory_policy_set':
        if (typeof params.log_conversations === 'boolean') {
          policy.log_conversations = params.log_conversations;
        }
        return policy;
      case 'memory_agents_list':
        return { root: policy.root, agents: [{ agent_id: 'main', turns: 4 }] };
      case 'memory_brain_ingest':
        return {
          id: `doc-${nextId++}`,
          source: (params.source as string) ?? 'markdown',
          replayed: false,
        };
      case 'memory_sources_list':
        return { sources: [...sources] };
      case 'memory_sources_add': {
        const source = {
          id: `src-${nextId++}`,
          kind: params.kind,
          target: params.target,
          label: (params.label as string | undefined) ?? params.target,
          schedule_mins: params.schedule_mins ?? null,
          last_sync_at: null,
          status: 'idle',
          error: null,
          items: 0,
        };
        sources.push(source);
        return { source };
      }
      case 'memory_sources_remove': {
        const index = sources.findIndex(s => s.id === params.id);
        if (index >= 0) sources.splice(index, 1);
        return { removed: index >= 0 };
      }
      case 'memory_sources_sync':
        return { started: params.id ? [params.id] : sources.map(s => s.id) };
      case 'memory_import_scan':
        return opts.importFound
          ? { found: true, counts: { documents: 12, conversations: 3, learnings: 5 } }
          : { found: false, counts: null };
      case 'memory_import_start':
        // A restart resumes from the persisted progress.
        importState = {
          phase: 'running',
          imported: importState.imported,
          total: 20,
          error: null,
          failed: 0,
        };
        return { state: importState };
      case 'memory_import_retry_failed':
        // The refused items go again; the next poll sees them stored.
        importState = { ...importState, phase: 'running', error: null };
        return { state: importState };
      case 'memory_import_status': {
        const current = importState;
        // A running import finishes on the next poll, so the page walks
        // idle -> running -> done without the spec waiting on real work.
        if (importState.phase === 'running') {
          importState = { phase: 'done', imported: 20, total: 20, error: null, failed: 0 };
        }
        return { state: current };
      }
      case 'memory_migration_scan':
        return {
          needed: Boolean(opts.migration) && !migrated,
          shared: Boolean(opts.migration?.shared),
        };
      case 'memory_migration_start':
        // A shared tree is never taken without consent.
        if (opts.migration?.shared && params.takeover !== true) {
          return { state: { phase: 'idle', copied: 0 }, running: false, interrupted: false };
        }
        migrating = true;
        return { state: { phase: 'copying', copied: 3 }, running: true, interrupted: false };
      case 'memory_migration_status': {
        // A running move finishes on the next poll.
        if (migrating) {
          migrating = false;
          migrated = true;
          return { state: { phase: 'copying', copied: 3 }, running: true, interrupted: false };
        }
        const phase = migrated ? 'cleaned' : 'idle';
        return { state: { phase, copied: migrated ? 5 : 0 }, running: false, interrupted: false };
      }
      default:
        return undefined;
    }
  };

  await page.route('**/rpc', async (route: Route, request: Request) => {
    let body: { id?: unknown; method?: string; params?: Record<string, unknown> } = {};
    try {
      body = JSON.parse(request.postData() || '{}');
    } catch {
      await route.continue();
      return;
    }
    const full = body.method ?? '';
    if (!full.startsWith('openhuman.memory_')) {
      await route.continue();
      return;
    }
    const method = full.slice('openhuman.'.length);
    const params = body.params ?? {};
    const failure = opts.failWith?.[method];
    if (failure) {
      calls.push({ method, params });
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          jsonrpc: '2.0',
          id: body.id,
          error: {
            code: -32000,
            message: failure.message,
            data: { code: failure.code, kind: failure.code },
          },
        }),
      });
      return;
    }
    const result = handle(method, params);
    if (result === undefined) {
      // Not part of the v2 surface: let the real core answer it.
      await route.continue();
      return;
    }
    calls.push({ method, params });
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ jsonrpc: '2.0', id: body.id, result }),
    });
  });

  return {
    calls,
    paramsOf: (method: string) => calls.filter(c => c.method === method).map(c => c.params),
  };
}

async function openMemory(page: Page, query = '') {
  const url = `${MEMORY_URL}${query}`;
  await page.goto(url);
  await waitForAppReady(page);
  // The shell can restore its persisted chat route once it is ready (the race
  // bootAuthenticatedPage also handles); reapply the Memory route if it did.
  if (!(await page.evaluate(() => window.location.hash)).startsWith('#/connections')) {
    await page.goto(url);
    await waitForAppReady(page);
  }
  await dismissWalkthroughIfPresent(page);
  await expect(page.getByTestId('memory-page')).toBeVisible({ timeout: 30_000 });
}

const hash = (page: Page) => page.evaluate(() => window.location.hash);

test.describe('Memory v2 — engine active', () => {
  test('lands on Provider and drives the three tabs: engine, conversations and files', async ({
    page,
  }) => {
    const fake = await installMemoryFake(page, { engineOn: true });
    await bootAuthenticatedPage(page, 'pw-memory-v2-active');
    await openMemory(page);

    // 1. No `?brain=` → the Provider chip, even with an active engine:
    // one CortexDB card, connected via TinyHumans.
    await expect(page.getByTestId('brain-tab-engine')).toHaveAttribute('aria-selected', 'true', {
      timeout: 20_000,
    });
    await expect(page.getByTestId('memory-engines')).toBeVisible();
    await expect(page.getByTestId('memory-engine-builtin')).toBeVisible();
    await expect(page.getByTestId('memory-engine-apikey')).toBeVisible();
    await expect(page.getByTestId('memory-engine-selfhost')).toBeVisible();
    await expect(page.getByTestId('memory-engine-status')).toHaveText('In use');
    await expect(page.getByTestId('memory-engine-chip-active-builtin')).toBeVisible();

    // The strip is exactly Provider, Conversations and Files.
    await expect(page.getByTestId('brain-tab-conversations')).toBeVisible();
    await expect(page.getByTestId('brain-tab-brain')).toHaveText('Files');
    for (const gone of ['migration', 'ask', 'explorer', 'learnings', 'background', 'settings']) {
      await expect(page.getByTestId(`brain-tab-${gone}`)).toHaveCount(0);
    }

    // 2. Conversations: the chat-logging controls render.
    await page.getByTestId('brain-tab-conversations').click();
    await expect(page.getByTestId('memory-conversations-tab')).toBeVisible();

    // 3. Files: upload button, then register a synced folder source (folder
    // is the default kind; only folder and file are offered).
    await page.getByTestId('brain-tab-brain').click();
    await expect(page.getByTestId('memory-brain-tab')).toBeVisible();
    await expect(page.getByTestId('memory-brain-add')).toBeVisible();
    // The per-source counts and brain search are gone.
    await expect(page.getByTestId('memory-brain-sources')).toHaveCount(0);
    await expect(page.getByTestId('memory-brain-search-input')).toHaveCount(0);
    await expect(page.getByTestId('memory-sources-empty')).toBeVisible();
    await page.getByTestId('memory-sources-add').click();
    await expect(page.getByTestId('memory-add-source')).toBeVisible();
    const kinds = await page
      .getByTestId('memory-add-source-kind')
      .locator('option')
      .evaluateAll(options => options.map(option => (option as HTMLOptionElement).value));
    expect(kinds).toEqual(['folder', 'file']);
    await page.getByTestId('memory-add-source-target').fill('/Users/e2e/notes');
    await page.getByTestId('memory-add-source-submit').click();
    await expect(page.getByTestId('memory-add-source')).toBeHidden();
    const row = page.getByTestId('memory-source-src-1');
    await expect(row).toBeVisible();
    await expect(row).toContainText('/Users/e2e/notes');
    expect(fake.paramsOf('memory_sources_add')).toEqual([
      { kind: 'folder', target: '/Users/e2e/notes' },
    ]);

    // 4. Upload: pasted text is ingested.
    await page.getByTestId('memory-brain-add').click();
    await page.getByTestId('memory-brain-ingest-text').fill('# Notes');
    await page.getByTestId('memory-brain-ingest-submit').click();
    await expect(page.getByTestId('memory-brain-notice')).toBeVisible();
    expect(fake.paramsOf('memory_brain_ingest')).toEqual([{ text: '# Notes' }]);
  });

  test('importing previous memory needs explicit consent', async ({ page }) => {
    const fake = await installMemoryFake(page, { engineOn: true, importFound: true });
    await bootAuthenticatedPage(page, 'pw-memory-v2-import');
    await openMemory(page, '&brain=brain');

    // 6. The scan found v1 data: the banner offers it with the counts.
    const banner = page.getByTestId('memory-import-banner');
    await expect(banner).toBeVisible({ timeout: 20_000 });
    await expect(page.getByTestId('memory-import-counts')).toContainText('12 documents');

    // Opening the offer only asks; nothing is uploaded yet.
    await page.getByTestId('memory-import-open').click();
    const consent = page.getByTestId('memory-import-consent');
    await expect(consent).toBeVisible();
    await expect(consent).toContainText('TinyHumans');
    expect(fake.paramsOf('memory_import_start')).toEqual([]);

    await page.getByTestId('memory-import-confirm').click();
    await expect(consent).toBeHidden();
    expect(fake.paramsOf('memory_import_start')).toEqual([{ consent: true }]);

    // Running first, then done once the status poll sees it finish.
    await expect(
      page.getByTestId('memory-import-running').or(page.getByTestId('memory-import-done'))
    ).toBeVisible();
    await expect(page.getByTestId('memory-import-done')).toBeVisible({ timeout: 15_000 });

    // The import lives on Files only; other chips carry no banner.
    await page.getByTestId('brain-tab-conversations').click();
    await expect(page.getByTestId('memory-conversations-tab')).toBeVisible();
    await expect(page.getByTestId('memory-import-banner')).toHaveCount(0);
  });

  test('a finished import retries the items it could not store', async ({ page }) => {
    const fake = await installMemoryFake(page, {
      engineOn: true,
      importFound: true,
      importFinishedWithFailed: 2,
    });
    await bootAuthenticatedPage(page, 'pw-memory-v2-import-retry');
    await openMemory(page, '&brain=brain');

    // Finished, with the refused items counted and a retry beside them.
    await expect(page.getByTestId('memory-import-failed-items')).toContainText('imported: 2', {
      timeout: 20_000,
    });
    await page.getByTestId('memory-import-retry-failed').click();
    // The click resolves before the RPC reaches the fake: wait for it.
    await expect.poll(() => fake.paramsOf('memory_import_retry_failed')).toEqual([{}]);
    expect(fake.paramsOf('memory_import_start')).toEqual([]);

    // Running, then done with nothing left to retry.
    await expect(page.getByTestId('memory-import-done')).toContainText('20 of 20', {
      timeout: 15_000,
    });
    await expect(page.getByTestId('memory-import-retry-failed')).toHaveCount(0);
  });

  test('a stopped import resumes only after consent', async ({ page }) => {
    const stopped =
      'not enough credits to import your memory; top up, then resume the import to continue where it stopped';
    const fake = await installMemoryFake(page, {
      engineOn: true,
      importFound: true,
      importStoppedWith: stopped,
    });
    await bootAuthenticatedPage(page, 'pw-memory-v2-import-resume');
    await openMemory(page, '&brain=brain');

    // The stopped import shows its reason and a Resume control, not the fresh offer.
    const failed = page.getByTestId('memory-import-error');
    await expect(failed).toBeVisible({ timeout: 20_000 });
    await expect(failed).toContainText('not enough credits');
    await expect(page.getByTestId('memory-import-open')).toHaveCount(0);

    // Resume asks again; nothing restarts until the user confirms.
    await page.getByTestId('memory-import-resume').click();
    const consent = page.getByTestId('memory-import-consent');
    await expect(consent).toBeVisible();
    expect(fake.paramsOf('memory_import_start')).toEqual([]);

    await page.getByTestId('memory-import-confirm').click();
    await expect(consent).toBeHidden();
    expect(fake.paramsOf('memory_import_start')).toEqual([{ consent: true }]);

    // It picks up from the stored progress and finishes.
    await expect(
      page.getByTestId('memory-import-running').or(page.getByTestId('memory-import-done'))
    ).toBeVisible();
    await expect(page.getByTestId('memory-import-done')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('memory-import-resume')).toHaveCount(0);
  });

  test('legacy Brain and settings links land on their v2 chips', async ({ page }) => {
    await installMemoryFake(page, { engineOn: true });
    await bootAuthenticatedPage(page, 'pw-memory-v2-legacy');

    const cases: Array<[string, RegExp]> = [
      ['/#/brain?tab=graph', /^#\/connections\?tab=brain&brain=conversations(?:&|$)/],
      ['/#/brain?tab=sources', /^#\/connections\?tab=brain&brain=brain(?:&|$)/],
      [`${MEMORY_URL}&brain=sync`, /^#\/connections\?tab=brain&brain=brain(?:&|$)/],
      ['/#/settings/memory-engine', /^#\/connections\?tab=brain&brain=engine(?:&|$)/],
      ['/#/settings/memory-data', /^#\/connections\?tab=brain&brain=brain(?:&|$)/],
      ['/#/settings/memory-debug', /^#\/connections\?tab=brain&brain=conversations(?:&|$)/],
      [`${MEMORY_URL}&brain=migration`, /^#\/connections\?tab=brain&brain=brain(?:&|$)/],
      [`${MEMORY_URL}&brain=ask`, /^#\/connections\?tab=brain&brain=conversations(?:&|$)/],
      [`${MEMORY_URL}&brain=explorer`, /^#\/connections\?tab=brain&brain=conversations(?:&|$)/],
      [`${MEMORY_URL}&brain=learnings`, /^#\/connections\?tab=brain&brain=conversations(?:&|$)/],
      [`${MEMORY_URL}&brain=background`, /^#\/connections\?tab=brain&brain=conversations(?:&|$)/],
      [`${MEMORY_URL}&brain=settings`, /^#\/connections\?tab=brain&brain=conversations(?:&|$)/],
    ];
    for (const [from, to] of cases) {
      await page.goto(from);
      await waitForAppReady(page);
      await expect.poll(() => hash(page), { message: `${from} should redirect` }).toMatch(to);
      await expect(page.getByTestId('memory-page')).toBeVisible();
    }
  });
});

test.describe('Memory v2 — memory off', () => {
  test('opens on Engine and every other chip shows the off state', async ({ page }) => {
    const fake = await installMemoryFake(page, { engineOn: false });
    await bootAuthenticatedPage(page, 'pw-memory-v2-off');
    await openMemory(page);

    // No usable engine and no `?brain=` → the Engine chip, with the off banner.
    await expect(page.getByTestId('brain-tab-engine')).toHaveAttribute('aria-selected', 'true', {
      timeout: 20_000,
    });
    await expect(page.getByTestId('memory-engine-status-off')).toBeVisible();
    await expect(page.getByTestId('memory-engines')).toBeVisible();

    // A content chip explains memory is off and points back to Engine.
    await page.goto(`${MEMORY_URL}&brain=brain`);
    await expect(page.getByTestId('memory-off-state')).toBeVisible({ timeout: 20_000 });
    await expect(page.getByTestId('memory-brain-add')).toHaveCount(0);
    await page.getByTestId('memory-off-open-engine').click();
    await expect.poll(() => hash(page)).toContain('brain=engine');
    await expect(page.getByTestId('memory-engine-tab')).toBeVisible();

    // Files explains memory is off too, instead of offering an import or upload.
    await page.getByTestId('brain-tab-brain').click();
    await expect(page.getByTestId('memory-brain-tab')).toBeVisible();
    await expect(page.getByTestId('memory-off-state')).toBeVisible();
    await expect(page.getByTestId('memory-brain-add')).toHaveCount(0);

    // With memory off the page never offers an import or lists sources.
    expect(fake.paramsOf('memory_import_scan')).toEqual([]);
    expect(fake.paramsOf('memory_sources_list')).toEqual([]);
  });
});

/**
 * A session token as the core stores it. The `local` signature marks the
 * offline "Set it up myself" session (`isLocalSessionToken`), which has no
 * TinyHumans account, so Built-in memory must not be selectable for it.
 */
function sessionToken(userId: string, signature: string): string {
  const payload = Buffer.from(
    JSON.stringify({ sub: userId, userId, exp: Math.floor(Date.now() / 1000) + 3600 })
  ).toString('base64url');
  return `eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.${payload}.${signature}`;
}

test.describe('Memory v2 — Engine tab connect flows', () => {
  test('TinyHumans connects with one click when signed in', async ({ page }) => {
    const fake = await installMemoryFake(page, { engineOn: false });
    await bootAuthenticatedPage(page, 'pw-memory-engine-builtin');
    // Stay on Engine: with no `?brain=` the page opens on Provider.
    await openMemory(page, '&brain=engine');

    // Nothing connected: the card opens on TinyHumans, one click away.
    const use = page.getByTestId('memory-engine-builtin-submit');
    await expect(use).toBeEnabled({ timeout: 20_000 });
    await use.click();
    await expect.poll(() => fake.paramsOf('memory_engine_set')).toEqual([{ engine: 'tinyhumans' }]);
    await expect(page.getByTestId('memory-engine-status')).toHaveText('In use');
    await expect(page.getByTestId('memory-engine-chip-active-builtin')).toBeVisible();
  });

  test('CortexDB with your API key connects with only a key', async ({ page }) => {
    const fake = await installMemoryFake(page, { engineOn: false });
    await bootAuthenticatedPage(page, 'pw-memory-engine-apikey');
    // Stay on Engine: with no `?brain=` the page opens on Provider.
    await openMemory(page, '&brain=engine');

    await page.getByTestId('memory-engine-apikey').click();
    await expect(page.getByTestId('memory-engine-panel-apikey')).toBeVisible();
    const submit = page.getByTestId('memory-engine-apikey-submit');
    await expect(submit).toBeDisabled(); // a key is required
    await page.getByTestId('memory-engine-apikey-key').fill('pw-cortex-key');
    await submit.click();
    // A blank endpoint clears any custom one, so the managed API is used.
    await expect
      .poll(() => fake.paramsOf('memory_engine_set'))
      .toEqual([{ engine: 'cortexdb', endpoint: '', api_key: 'pw-cortex-key' }]);
    await expect(page.getByTestId('memory-engine-status')).toHaveText('In use');
    await expect(page.getByTestId('memory-engine-chip-active-apikey')).toBeVisible();
  });

  test('CortexDB Local refuses a non-local endpoint and connects a local one', async ({ page }) => {
    const fake = await installMemoryFake(page, { engineOn: false });
    await bootAuthenticatedPage(page, 'pw-memory-engine-selfhost');
    // Stay on Engine: with no `?brain=` the page opens on Provider.
    await openMemory(page, '&brain=engine');

    await page.getByTestId('memory-engine-selfhost').click();
    await expect(page.getByTestId('memory-engine-selfhost-docs')).toBeVisible();
    await page.getByTestId('memory-engine-selfhost-key').fill('pw-local-key');

    // Not on this computer: refused before any RPC.
    await page.getByTestId('memory-engine-selfhost-endpoint').fill('http://192.168.1.10:3141');
    await expect(page.getByTestId('memory-engine-selfhost-endpoint-error')).toContainText(
      'Self-hosting is local only'
    );
    await expect(page.getByTestId('memory-engine-selfhost-submit')).toBeDisabled();
    expect(fake.paramsOf('memory_engine_set')).toEqual([]);

    // Loopback: accepted and sent with the key.
    await page.getByTestId('memory-engine-selfhost-endpoint').fill('http://localhost:3141');
    await expect(page.getByTestId('memory-engine-selfhost-endpoint-error')).toHaveCount(0);
    await page.getByTestId('memory-engine-selfhost-submit').click();
    await expect
      .poll(() => fake.paramsOf('memory_engine_set'))
      .toEqual([
        { engine: 'cortexdb', endpoint: 'http://localhost:3141', api_key: 'pw-local-key' },
      ]);
    await expect(page.getByTestId('memory-engine-status')).toHaveText('In use');
    await expect(page.getByTestId('memory-engine-chip-active-selfhost')).toBeVisible();
  });

  test('TinyHumans is not selectable without a TinyHumans account', async ({ page }) => {
    const fake = await installMemoryFake(page, { engineOn: false });
    // A "Set it up myself" (local) session that finished onboarding.
    await bootRuntimeReadyGuestPage(page);
    // The core requires the local user payload, as Welcome's "Set it up
    // myself" stores it (utils/localSession.ts LOCAL_SESSION_USER).
    await callCoreRpc('openhuman.auth_store_session', {
      token: sessionToken('local', 'local'),
      user: { _id: 'local', id: 'local', name: 'Local User', email: 'local@openhuman.local' },
    });
    await callCoreRpc('openhuman.config_set_onboarding_completed', { value: true });
    await page.goto(`${MEMORY_URL}&brain=engine`);
    await page.reload();
    await waitForAppReady(page);
    // The shell restores its persisted chat route once ready; reapply the
    // Memory route afterwards, as bootAuthenticatedPage does.
    await page.goto(`${MEMORY_URL}&brain=engine`);
    await waitForAppReady(page);
    await dismissWalkthroughIfPresent(page);
    await expect(page.getByTestId('memory-page')).toBeVisible({ timeout: 30_000 });

    await expect(page.getByTestId('memory-engine-builtin-sign-in')).toBeVisible({
      timeout: 20_000,
    });
    await expect(page.getByTestId('memory-engine-builtin-submit')).toBeDisabled();
    expect(fake.paramsOf('memory_engine_set')).toEqual([]);
  });
});

test.describe('Memory v2 — out of credits', () => {
  test('an upload out of credits prompts a top-up that lands on billing', async ({ page }) => {
    const fake = await installMemoryFake(page, {
      engineOn: true,
      failWith: { memory_brain_ingest: OUT_OF_CREDITS },
    });
    await bootAuthenticatedPage(page, 'pw-memory-v2-credits-upload');
    await openMemory(page, '&brain=brain');

    await page.getByTestId('memory-brain-add').click({ timeout: 20_000 });
    await page.getByTestId('memory-brain-ingest-text').fill('# Notes');
    await page.getByTestId('memory-brain-ingest-submit').click();

    // A prompt, not an error: warning variant, the top-up explanation, no raw 402.
    const prompt = page.getByTestId('memory-brain-ingest-error');
    await expect(prompt).toHaveAttribute('data-kind', 'out-of-credits', { timeout: 20_000 });
    await expect(prompt).toContainText('Out of credits');
    await expect(prompt).not.toContainText('HTTP 402');
    expect(fake.paramsOf('memory_brain_ingest')).toEqual([{ text: '# Notes' }]);

    // Top up goes to the billing page.
    await page.getByTestId('memory-top-up').click();
    await expect.poll(() => hash(page)).toContain('#/settings/account');
  });
});

test.describe('Memory v2 — move into the per-user layout', () => {
  test('migrate now moves the legacy memory and the banner goes away', async ({ page }) => {
    const fake = await installMemoryFake(page, { engineOn: true, migration: { shared: false } });
    await bootAuthenticatedPage(page, 'pw-memory-v2-migrate');
    await openMemory(page, '&brain=brain');

    await expect(page.getByTestId('memory-migration-offer')).toBeVisible({ timeout: 20_000 });
    await page.getByTestId('memory-migration-start').click();
    await expect.poll(() => fake.paramsOf('memory_migration_start')).toEqual([{ takeover: false }]);
    await expect(page.getByTestId('memory-migration-banner')).toBeHidden({ timeout: 15_000 });
  });

  test('a tree other accounts may share is taken only after consent', async ({ page }) => {
    const fake = await installMemoryFake(page, { engineOn: true, migration: { shared: true } });
    await bootAuthenticatedPage(page, 'pw-memory-v2-migrate-takeover');
    await openMemory(page, '&brain=brain');

    await page.getByTestId('memory-migration-start').click({ timeout: 20_000 });
    const takeover = page.getByTestId('memory-migration-takeover');
    await expect(takeover).toBeVisible();
    expect(fake.paramsOf('memory_migration_start')).toEqual([]);

    await page.getByTestId('memory-migration-takeover-cancel').click();
    await expect(takeover).toBeHidden();
    expect(fake.paramsOf('memory_migration_start')).toEqual([]);

    await page.getByTestId('memory-migration-start').click();
    await page.getByTestId('memory-migration-takeover-confirm').click();
    await expect(takeover).toBeHidden();
    await expect.poll(() => fake.paramsOf('memory_migration_start')).toEqual([{ takeover: true }]);
    await expect(page.getByTestId('memory-migration-banner')).toBeHidden({ timeout: 15_000 });
  });
});
