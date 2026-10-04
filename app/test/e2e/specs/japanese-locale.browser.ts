import { expect, type Page, test } from '@playwright/test';

import {
  bootAuthenticatedPage,
  dismissWalkthroughIfPresent,
} from '../../playwright/helpers/core-rpc';

async function settings(page: Page, user: string) {
  await bootAuthenticatedPage(page, user, '/settings/account');
  await dismissWalkthroughIfPresent(page);
}

async function expectJapanese(page: Page) {
  await expect(page.getByRole('combobox', { name: '言語', exact: true })).toHaveValue('ja');
  await expect(page.locator('html')).toHaveAttribute('lang', 'ja');
  await expect(page.locator('html')).toHaveAttribute('dir', 'ltr');
  await expect(page.locator('[data-walkthrough="tab-chat"]')).toContainText('チャット');
}

test.describe('Japanese UI locale', () => {
  test.describe.configure({ timeout: 180_000 });

  test('selects Japanese through Settings, restores it after reload, and switches back', async ({
    page,
  }) => {
    await settings(page, 'pw-japanese-switch');
    await page
      .getByRole('combobox', { name: 'Language', exact: true })
      .selectOption({ label: '🇯🇵 日本語' });
    await expectJapanese(page);
    // Wait for redux-persist to write, rather than dispatching or seeding a locale.
    await expect
      .poll(() =>
        page.evaluate(() => Object.values(localStorage).some(value => value.includes('\\"ja\\"')))
      )
      .toBe(true);
    await page.reload();
    await expectJapanese(page);
    await page.getByRole('combobox', { name: '言語', exact: true }).selectOption('en');
    await expect(page.getByRole('combobox', { name: 'Language', exact: true })).toHaveValue('en');
    await expect(page.locator('html')).toHaveAttribute('lang', 'en');
    await expect(page.locator('[data-walkthrough="tab-chat"]')).toContainText('Chat');
    await expect
      .poll(() =>
        page.evaluate(() => Object.values(localStorage).some(value => value.includes('\\"en\\"')))
      )
      .toBe(true);
    await page.reload();
    await expect(page.getByRole('combobox', { name: 'Language', exact: true })).toHaveValue('en');
  });

  test('detects ja-JP on a fresh browser and preserves a manual English override', async ({
    browser,
  }) => {
    const context = await browser.newContext({
      locale: 'ja-JP',
      viewport: { width: 1280, height: 720 },
    });
    const page = await context.newPage();
    try {
      await settings(page, 'pw-japanese-detect');
      expect(await page.evaluate(() => navigator.language)).toBe('ja-JP');
      await expectJapanese(page);
      await expect(page.getByRole('combobox', { name: '言語', exact: true })).toBeInViewport();
      await page.getByRole('combobox', { name: '言語', exact: true }).selectOption('en');
      await expect
        .poll(() =>
          page.evaluate(() => Object.values(localStorage).some(value => value.includes('\\"en\\"')))
        )
        .toBe(true);
      await page.reload();
      await expect(page.getByRole('combobox', { name: 'Language', exact: true })).toHaveValue('en');
      expect(await page.evaluate(() => navigator.language)).toBe('ja-JP');
    } finally {
      await context.close();
    }
  });

  test('renders Japanese memory counts and model attribution at the default viewport', async ({
    page,
  }) => {
    const fixtures: Record<string, unknown> = {
      'openhuman.memory_engine_get': {
        result: { engine: 'tinycortex', has_key: true, status: 'ready', fetch_modes: [] },
        logs: [],
      },
      'openhuman.memory_engines_list': { result: { engines: [] }, logs: [] },
      'openhuman.memory_explore': {
        result: {
          facet: 'kind',
          buckets: [{ value: 'document', count: 7 }],
          total: 7,
          missing: 0,
          more_buckets: 0,
          truncated: false,
        },
        logs: [],
      },
      'openhuman.memory_items_list': { result: { items: [] }, logs: [] },
      'openhuman.tokenjuice_savings_stats': {
        attributionModel: 'Qwen3.8-Flash-Next',
        total: {
          events: 3,
          originalTokens: 1000,
          compactedTokens: 500,
          tokensSaved: 500,
          costSavedUsd: 0,
        },
        byModel: {},
        byCompressor: {},
        cache: { entries: 0, bytes: 0 },
      },
    };
    // Only data-dependent read results are fixtures; authentication, locale
    // persistence and navigation still run through the shared Core harness.
    await page.route('**/rpc', async route => {
      const request = route.request().postDataJSON();
      if (request && Object.prototype.hasOwnProperty.call(fixtures, request.method)) {
        await route.fulfill({
          json: { jsonrpc: '2.0', id: request.id, result: fixtures[request.method] },
        });
      } else {
        await route.continue();
      }
    });
    await page.setViewportSize({ width: 1280, height: 720 });
    await settings(page, 'pw-japanese-interpolation');
    await page.getByRole('combobox', { name: 'Language', exact: true }).selectOption('ja');
    await page.goto('/#/connections?tab=brain&brain=explorer');
    const count = page.getByText('この場所の項目: 7件', { exact: true });
    await expect(count).toBeVisible();
    await count.scrollIntoViewIfNeeded();
    await expect(count).toBeInViewport();
    await page.goto('/#/connections?tab=usage#tokens');
    const attribution = page.getByText('Qwen3.8-Flash-Next のコストで計算', { exact: true });
    await expect(attribution).toBeVisible();
    await attribution.scrollIntoViewIfNeeded();
    await expect(attribution).toBeInViewport();
    await expect(page.getByText('3 回の圧縮で', { exact: true })).toBeVisible();
  });
});
