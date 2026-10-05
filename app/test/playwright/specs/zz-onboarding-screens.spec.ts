/**
 * Screenshot capture of every onboarding screen, light and dark.
 *
 * Not an assertion suite — it walks the flow and writes a PNG per screen so a
 * human can eyeball the restyle. Named `zz-` so it runs last.
 */
import { type Page, test } from '@playwright/test';

import { bootAuthenticatedPage, callCoreRpc, waitForAppReady } from '../helpers/core-rpc';

const OUT = 'test-results/onboarding-screens';

async function bootIntoOnboarding(page: Page, userId: string): Promise<void> {
  await bootAuthenticatedPage(page, userId, '/home');
  await callCoreRpc('openhuman.config_set_onboarding_completed', { value: false });
  await page.goto('/#/onboarding/welcome');
  await waitForAppReady(page);
}

async function go(page: Page, hash: string, stepTestId?: string): Promise<void> {
  await page.goto(`/#${hash}`);
  await waitForAppReady(page);
  if (stepTestId) {
    await page.getByTestId(stepTestId).waitFor({ state: 'visible', timeout: 30_000 });
  }
}

async function setTheme(page: Page, dark: boolean): Promise<void> {
  await page.evaluate(d => {
    document.documentElement.classList.toggle('dark', d);
  }, dark);
  await page.waitForTimeout(250);
}

async function shoot(page: Page, name: string): Promise<void> {
  await page.waitForTimeout(400);
  await page.screenshot({ path: `${OUT}/${name}.png`, fullPage: true });
}

test.describe('Onboarding screens', () => {
  test('captures every screen in light and dark', async ({ page }) => {
    test.setTimeout(180_000);
    await bootIntoOnboarding(page, 'screens-user');

    for (const dark of [false, true]) {
      const mode = dark ? 'dark' : 'light';

      await go(page, '/onboarding/welcome', 'onboarding-welcome-step');
      await setTheme(page, dark);
      await shoot(page, `01-welcome-${mode}`);

      await go(page, '/onboarding/runtime-choice', 'onboarding-runtime-choice-step');
      await setTheme(page, dark);
      await shoot(page, `02-runtime-choice-${mode}`);

      await go(page, '/onboarding/custom/inference', 'onboarding-custom-inference-step');
      await setTheme(page, dark);
      await shoot(page, `03-inference-${mode}`);
      await shoot(page, `04-inference-configure-${mode}`);

      await go(page, '/onboarding/custom/search', 'onboarding-custom-search-step');
      await setTheme(page, dark);
      await shoot(page, `05-search-${mode}`);
      await shoot(page, `06-search-configure-${mode}`);

      await go(page, '/onboarding/custom/vault', 'onboarding-custom-vault-step');
      await setTheme(page, dark);
      await shoot(page, `07-vault-${mode}`);
      await shoot(page, `08-vault-configure-${mode}`);
    }

    // Narrow viewport — the p-10 -> p-6 sm:p-8 change targeted row wrapping.
    await page.setViewportSize({ width: 400, height: 900 });
    await go(page, '/onboarding/custom/inference', 'onboarding-custom-inference-step');
    await setTheme(page, false);
    await shoot(page, '09-inference-narrow-400px');
    await go(page, '/onboarding/custom/search', 'onboarding-custom-search-step');
    await shoot(page, '10-search-narrow-400px');
  });
});
