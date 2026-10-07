import { browser, expect } from '@wdio/globals';

import { waitForApp } from '../helpers/app-helpers';
import { waitForTestId } from '../helpers/element-helpers';
import { resetApp } from '../helpers/reset-app';
import { navigateViaHash } from '../helpers/shared-flows';
import { startMockServer, stopMockServer } from '../mock-server';

type Tab = 'providers' | 'routing' | 'websites';
const TABS: Tab[] = ['providers', 'routing', 'websites'];

async function expectSelectedAndFocused(selected: Tab): Promise<void> {
  for (const tab of TABS) {
    const element = await waitForTestId(`search-tab-${tab}`);
    await expect(element).toHaveAttribute('aria-selected', String(tab === selected));
    if (tab === selected) await expect(element).toBeFocused();
  }
}

// Registered by app/scripts/e2e-run-all-flows.sh in the navigation suite.
describe('ChipTabs keyboard traversal', () => {
  before(async () => {
    await startMockServer();
    await waitForApp();
    await resetApp('e2e-chip-tabs-keyboard');
  });

  after(async () => {
    await stopMockServer();
  });

  it('enters on the active tab and selects with arrows, Home, End and wrapping', async () => {
    await navigateViaHash('/settings/search');
    await waitForTestId('search-settings-panel');
    // Start in the middle so each jump changes the controlled selection.
    const middle = await waitForTestId('search-tab-routing');
    await middle.click();
    await expectSelectedAndFocused('routing');

    // Leave the row, then enter with a real Tab event. This exercises entry
    // focus in Radix's roving group before the arrow-key assertions.
    await browser.keys(['Shift', 'Tab']);
    await expect(middle).not.toBeFocused();
    await browser.keys('Tab');
    await expectSelectedAndFocused('routing');

    for (const [key, selected] of [
      ['ArrowRight', 'websites'],
      ['ArrowLeft', 'routing'],
      ['Home', 'providers'],
      ['End', 'websites'],
      ['ArrowRight', 'providers'],
      ['ArrowLeft', 'websites'],
    ] as const) {
      await browser.keys(key);
      await expectSelectedAndFocused(selected);
    }
  });
});
