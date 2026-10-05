import { expect, type Page, test } from '@playwright/test';

import { bootRuntimeReadyGuestPage, callCoreRpc, waitForAppReady } from '../helpers/core-rpc';

const MOCK_ADMIN_BASE = `http://127.0.0.1:${process.env.E2E_MOCK_PORT || '18473'}`;

async function resetMock(): Promise<void> {
  await fetch(`${MOCK_ADMIN_BASE}/__admin/reset`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({}),
  });
}

async function readOnboardingCompleted(): Promise<boolean> {
  const completed = await callCoreRpc<boolean | { result?: boolean }>(
    'openhuman.config_get_onboarding_completed',
    {}
  );
  const value = typeof completed === 'boolean' ? completed : completed?.result;
  // Require a real boolean so a response-shape change fails loudly instead of
  // being read as "not completed".
  expect(typeof value).toBe('boolean');
  return value as boolean;
}

/**
 * Store a real (non-local) TinyHumans session with `onboarding_completed=false`
 * and open the onboarding entry route.
 *
 * `/onboarding/welcome` no longer renders anything interactive: for a real
 * session it marks onboarding complete and routes to `/chat`, so there is no
 * onboarding screen to wait for. Callers assert the landing instead.
 */
async function signInThroughOnboarding(page: Page, userId: string): Promise<void> {
  const payload = Buffer.from(
    JSON.stringify({ sub: userId, userId, exp: Math.floor(Date.now() / 1000) + 3600 })
  ).toString('base64url');
  const token = `eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.${payload}.sig`;
  await callCoreRpc('openhuman.auth_store_session', { token });
  await callCoreRpc('openhuman.config_set_onboarding_completed', { value: false });
  await page.goto('/#/onboarding/welcome');
  await waitForAppReady(page);
}

async function expectLandedInChat(page: Page): Promise<void> {
  await expect
    .poll(async () => page.evaluate(() => window.location.hash), { timeout: 20_000 })
    .toMatch(/^#\/chat/);
  await expect.poll(readOnboardingCompleted, { timeout: 20_000 }).toBe(true);
  // Nothing of the onboarding stepper may linger once the user is in the app.
  await expect(page.getByTestId('onboarding-layout')).toHaveCount(0);
}

async function logoutViaSettings(page: Page): Promise<void> {
  await callCoreRpc('openhuman.auth_clear_session', {});
  await page.goto('/#/');
  // A core RPC response only confirms that persistence was updated. Wait for
  // the browser's CoreStateProvider to observe that signed-out snapshot before
  // asserting the public route, otherwise a preceding authenticated snapshot
  // can win the reload race in a busy serial CI lane.
  await expect
    .poll(() =>
      page.evaluate(() => {
        const state = (
          window as typeof window & {
            __OPENHUMAN_CORE_STATE__?: () => { snapshot?: { sessionToken?: string | null } };
          }
        ).__OPENHUMAN_CORE_STATE__?.();
        return Boolean(state?.snapshot?.sessionToken);
      })
    )
    .toBe(false);
}

test.describe('Logout -> re-login onboarding overlay', () => {
  test.beforeEach(async ({ page }) => {
    await resetMock();
    await bootRuntimeReadyGuestPage(page);
  });

  test('re-login after logout re-runs onboarding from clean state and lands in chat', async ({
    page,
  }) => {
    // First login: onboarding resolves on its own for a TinyHumans session.
    await signInThroughOnboarding(page, 'pw-logout-relogin-user');
    await expectLandedInChat(page);

    // Logout drops the session and shows the signed-out Welcome screen, not a
    // stale onboarding or chat surface.
    await logoutViaSettings(page);
    await expect(page.getByTestId('welcome-cta-tinyhumans')).toBeVisible({ timeout: 20_000 });
    await expect(page.getByTestId('onboarding-layout')).toHaveCount(0);

    // Re-login with onboarding reset: the flow must run again from the start
    // (not be skipped by leftover state from the first session, and not wedge
    // on a screen) and end in chat with the flag set again.
    await callCoreRpc('openhuman.config_set_onboarding_completed', { value: false });
    expect(await readOnboardingCompleted()).toBe(false);

    await signInThroughOnboarding(page, 'pw-logout-relogin-user');
    await expectLandedInChat(page);
  });
});
