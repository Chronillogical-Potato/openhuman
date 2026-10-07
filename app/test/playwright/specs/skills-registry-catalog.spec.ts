import { expect, type Page, test } from '@playwright/test';

import { setMockBehavior } from '../helpers/chat-drive';
import {
  bootAuthenticatedPage,
  callCoreRpc,
  dismissWalkthroughIfPresent,
  waitForAppReady,
} from '../helpers/core-rpc';

const CATALOG_SIZE = 32;
const PAGE_SIZE = 25;

async function openRegistry(page: Page, userId: string) {
  await page.route('**/rpc', async (route, request) => {
    try {
      const body = JSON.parse(request.postData() || '{}');
      if (body.method === 'openhuman.composio_list_agent_ready_toolkits') {
        await route.fulfill({
          contentType: 'application/json',
          body: JSON.stringify({
            jsonrpc: '2.0',
            id: body.id,
            result: { result: { toolkits: [] }, logs: [] },
          }),
        });
        return;
      }
    } catch {
      /* pass through */
    }
    await route.continue();
  });
  await bootAuthenticatedPage(page, userId, '/connections?tab=skills');
  await expect
    .poll(() => page.evaluate(() => window.location.hash), { timeout: 15_000 })
    .toContain('tab=skills');
  await waitForAppReady(page);
  await dismissWalkthroughIfPresent(page);
  await page.getByTestId('skill-explorer-tab-registry').click();
  await expect(page.getByTestId('skill-search-input')).toBeVisible({ timeout: 20_000 });
}

const catalogRows = (page: Page) => page.locator('[data-testid^="registry-tile-"]');

test.describe('Skill registry catalog over the tinyskills registry', () => {
  test('pages, filters and reports freshness over JSON-RPC', async () => {
    test.setTimeout(60_000);
    const first = await callCoreRpc<{
      entries: Array<{ id: string; installable: boolean; registry: string }>;
      total: number;
      page: number;
      total_pages: number;
      freshness: string;
      last_error: unknown;
    }>('openhuman.skill_registry_browse', { page: 1, page_size: PAGE_SIZE, force_refresh: true });
    expect(first.total).toBe(CATALOG_SIZE);
    expect(first.entries).toHaveLength(PAGE_SIZE);
    expect(first.total_pages).toBe(2);
    expect(first.freshness).toBe('live');
    expect(first.last_error).toBeNull();
    expect(first.entries.every(entry => entry.installable && entry.registry === 'hermes')).toBe(
      true
    );

    const second = await callCoreRpc<{ entries: unknown[]; page: number }>(
      'openhuman.skill_registry_browse',
      { page: 2, page_size: PAGE_SIZE }
    );
    expect(second.page).toBe(2);
    expect(second.entries).toHaveLength(CATALOG_SIZE - PAGE_SIZE);

    const pack = await callCoreRpc<{ total: number }>('openhuman.skill_registry_browse', {
      page: 1,
      page_size: PAGE_SIZE,
      sources: ['fixture-pack'],
    });
    expect(pack.total).toBe(30);

    const detail = await callCoreRpc<{ id: string; overview: string; download_url: string }>(
      'openhuman.skill_registry_detail',
      { entry_id: 'docker-management' }
    );
    expect(detail.id).toBe('docker-management');
    expect(detail.download_url).toContain('/skills/docker-management/SKILL.md');
  });

  test('browses page by page, installs a skill and uninstalls it', async ({ page }) => {
    test.setTimeout(90_000);
    await openRegistry(page, 'pw-skills-catalog-lifecycle');

    await expect(catalogRows(page)).toHaveCount(PAGE_SIZE, { timeout: 30_000 });
    const pager = page.getByTestId('registry-pagination');
    await pager.getByRole('button', { name: 'Next page' }).click();
    await expect(catalogRows(page)).toHaveCount(CATALOG_SIZE - PAGE_SIZE, { timeout: 15_000 });

    await page.getByTestId('skill-search-input').fill('docker');
    const install = page.getByTestId('registry-install-docker-management');
    await expect(install).toBeVisible({ timeout: 15_000 });
    await install.click();
    await expect(
      page.getByTestId('registry-tile-docker-management').getByText('Installed', { exact: true })
    ).toBeVisible({ timeout: 30_000 });

    await page.getByTestId('skill-explorer-tab-installed').click();
    const uninstall = page.getByTestId('skill-uninstall-docker-management');
    await expect(uninstall).toBeVisible({ timeout: 15_000 });
    await uninstall.click();
    await page.getByTestId('uninstall-skill-confirm').click();
    await expect(uninstall).toHaveCount(0, { timeout: 15_000 });
  });

  test('keeps showing the saved catalog when the registry goes offline', async ({ page }) => {
    test.setTimeout(90_000);
    await openRegistry(page, 'pw-skills-catalog-offline');
    await expect(catalogRows(page)).toHaveCount(PAGE_SIZE, { timeout: 30_000 });

    try {
      await setMockBehavior('skillRegistryUnavailable', 'true');
      await page.getByRole('button', { name: 'Refresh registry' }).click();

      await expect(page.getByTestId('registry-offline')).toBeVisible({ timeout: 30_000 });
      await expect(catalogRows(page)).toHaveCount(PAGE_SIZE);

      await page.getByTestId('registry-retry').click();
      await expect(page.getByTestId('registry-offline')).toBeVisible({ timeout: 30_000 });
      await expect(catalogRows(page)).toHaveCount(PAGE_SIZE);
    } finally {
      await setMockBehavior('skillRegistryUnavailable', 'false');
    }
  });
});
