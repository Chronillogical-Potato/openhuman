import { expect, type Page, test } from '@playwright/test';

import {
  callCoreRpc,
  dismissWalkthroughIfPresent,
  seedBrowserCoreMode,
  waitForAppReady,
} from '../helpers/core-rpc';

async function resetOnboarding(userId: string): Promise<void> {
  await callCoreRpc('openhuman.auth_clear_session', {});
  await callCoreRpc('openhuman.config_set_onboarding_completed', { value: false });
  await callCoreRpc('openhuman.auth_store_session', {
    token: `eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.${Buffer.from(
      JSON.stringify({ sub: userId, userId, exp: Math.floor(Date.now() / 1000) + 3600 })
    ).toString('base64url')}.sig`,
  });
}

async function chooseConfigure(page: Page): Promise<void> {
  const configure = page.getByRole('radio', { name: /Configure/ }).first();
  if (await configure.isVisible().catch(() => false)) {
    await configure.click();
  }
}

test.describe('Onboarding custom configuration flow', () => {
  test('advanced path supports configure choices and back navigation through final setup', async ({
    page,
  }) => {
    await resetOnboarding('pw-onboarding-config');
    await seedBrowserCoreMode(page);
    await page.goto('/#/onboarding/welcome');
    await waitForAppReady(page);
    await dismissWalkthroughIfPresent(page);

    await expect(page.getByTestId('onboarding-welcome-step')).toBeVisible();
    await page.getByRole('button', { name: 'Get Started' }).click();
    await expect(page.getByTestId('onboarding-runtime-choice-step')).toBeVisible();
    await page.getByTestId('onboarding-runtime-choice-custom').click();
    await page.getByTestId('onboarding-next-button').click();

    await expect(page.getByTestId('onboarding-custom-inference-step')).toBeVisible();
    await chooseConfigure(page);
    await page.getByTestId('onboarding-next-button').click();

    await expect(page.getByTestId('onboarding-custom-search-step')).toBeVisible();
    await chooseConfigure(page);
    await page.getByRole('button', { name: /Back/ }).click();
    await expect(page.getByTestId('onboarding-custom-inference-step')).toBeVisible();
    await page.getByTestId('onboarding-next-button').click();

    await expect(page.getByTestId('onboarding-custom-search-step')).toBeVisible({
      timeout: 20_000,
    });
    // Search is optional: skipping goes straight to the final step.
    await page.getByTestId('onboarding-search-skip').click();

    await expect(page.getByTestId('onboarding-custom-vault-step')).toBeVisible({ timeout: 20_000 });
    await chooseConfigure(page);
    await expect(page.getByTestId('onboarding-next-button')).toBeEnabled();
  });
});
