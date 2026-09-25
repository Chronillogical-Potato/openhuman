import { expect, test } from '@playwright/test';

import {
  bootRuntimeReadyGuestPage,
  dismissWalkthroughIfPresent,
  signInViaBypassUser,
  waitForAppReady,
} from '../helpers/core-rpc';

const MOCK_ADMIN_BASE = `http://127.0.0.1:${process.env.E2E_MOCK_PORT || '18473'}`;

interface MockRequest {
  method: string;
  url: string;
}

async function resetMock(): Promise<void> {
  await fetch(`${MOCK_ADMIN_BASE}/__admin/reset`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({}),
  });
}

async function mockRequests(): Promise<MockRequest[]> {
  const response = await fetch(`${MOCK_ADMIN_BASE}/__admin/requests`);
  const payload = (await response.json()) as { data?: MockRequest[] };
  return Array.isArray(payload.data) ? payload.data : [];
}

async function waitForMockRequest(method: string, pathFragment: string, timeoutMs = 15_000) {
  const deadline = Date.now() + timeoutMs;
  let delay = 200;
  while (Date.now() < deadline) {
    const match = (await mockRequests()).find(
      request => request.method === method && request.url.includes(pathFragment)
    );
    if (match) return match;
    await new Promise(resolve => setTimeout(resolve, delay));
    delay = Math.min(delay * 1.5, 1_000);
  }
  return null;
}

test.describe('Runtime picker -> login -> logout', () => {
  test.beforeEach(async ({ page }) => {
    await resetMock();
    await bootRuntimeReadyGuestPage(page);
  });

  // DELETED, not unskipped: `runtime picker validates cloud URL/token inputs
  // and unreachable hosts` and `returning to cloud-mode guest state keeps
  // provider login available`.
  //
  // Both carried `test.skip(true, 'web Playwright lane does not reliably
  // surface the desktop-style runtime picker overlay yet')`. That reason is
  // structural, not flaky, and no amount of rewriting fixes it: every test in
  // this file boots through `bootRuntimeReadyGuestPage`, which calls
  // `seedBrowserCoreMode` and writes `openhuman_core_mode`,
  // `openhuman_core_rpc_url` and `openhuman_core_rpc_token` into localStorage
  // before the first paint. The app therefore already knows its runtime and
  // is correct not to show the picker. Removing that seeding leaves the web
  // build with no core to talk to, so the page never boots — the overlay is
  // unreachable in this lane by construction.
  //
  // The desktop lane covers all of it, and more: see
  // `app/test/e2e/specs/runtime-picker-login.spec.ts` —
  //   `clicking "Select a Runtime" opens the runtime picker with both options`,
  //   `cloud option reveals URL + token inputs and validates them`,
  //   `"Test Connection" against an unreachable host shows the unreachable pill`,
  //   `switching back to Local and clicking Continue closes the picker`.
  // That is a strict superset of the two deleted cases.

  test('provider login reaches home and logout returns to welcome', async ({ page }) => {
    await signInViaBypassUser(page, 'pw-runtime-picker-login');
    await dismissWalkthroughIfPresent(page);

    await expect
      .poll(async () => page.evaluate(() => window.location.hash))
      .toMatch(/^#\/(home|chat)(\/|$)/);
    await expect(await waitForMockRequest('GET', '/auth/me')).toBeTruthy();

    await page.goto('/#/settings/account');
    await waitForAppReady(page);
    await page.getByTestId('settings-nav-logout').click();

    await expect(page.getByText('Welcome to OpenHuman')).toBeVisible();
  });
});
