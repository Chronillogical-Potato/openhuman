import { expect } from '@wdio/globals';

import { waitForApp } from '../helpers/app-helpers';
import { waitForTestId } from '../helpers/element-helpers';
import { resetApp } from '../helpers/reset-app';
import { navigateViaHash } from '../helpers/shared-flows';

/**
 * External navigation — a remote page never loads in the main webview.
 *
 * Matrix 4.2.11 is 🟡 and says *"automated desktop E2E is a follow-up (the WDIO
 * desktop runner still assumes CEF)"*. **That parenthetical is stale.** The
 * Appium/CEF backend was removed in #5478 precisely because CDP does not exist
 * under Wry; the runner is a single `tauri-driver` WebDriver session against
 * the native WebKit webview (`app/scripts/e2e-run-session.sh:14-15`,
 * `app/test/wdio.conf.ts:10-14`), and the shell is
 * `pub(crate) type AppRuntime = tauri::Wry` unconditionally
 * (`crates/openhuman-app/src/lib.rs:131`). The stated blocker is gone.
 *
 * This is the one case in this slice that cannot be a Playwright spec. The
 * claim is about the *shell's* policy: `external_navigation::init()`'s
 * `on_navigation` hook returns `false` for any http(s) navigation of the `main`
 * window that is not the app origin or the dev server, cancelling it and handing
 * the URL to the OS opener (`crates/openhuman-app/src/external_navigation.rs:54-89`).
 * A browser has no such hook, so a Playwright version would assert nothing.
 *
 * RU (`external_navigation_tests.rs`) proves `navigation_handoff` returns the
 * right answer. It cannot prove the plugin is *installed on the main window in
 * the shipping build* — a wiring regression (registered late, or on the wrong
 * webview) passes every existing test while the main webview, which holds the
 * app's IPC bridge, loads someone else's origin. That is a security boundary,
 * not a UX defect, which is why it is worth a desktop spec of its own.
 *
 * ## No network leaves this machine
 *
 * The probe host is under `.invalid`, the TLD RFC 2606 reserves as guaranteed
 * never to resolve. The assertion is that navigation did NOT happen, so the URL
 * never needs to load; and when the policy works as intended it hands the URL to
 * the OS opener, whose own lookup fails at DNS. Nothing contacts a real service
 * on either branch.
 */

const USER_ID = 'e2e-chat-external-link';
const REMOTE_URL = 'https://openhuman-e2e.invalid/external-link-probe';

/** The app's own URL, whatever scheme the shell serves it under. */
async function currentUrl(): Promise<string> {
  return browser.getUrl();
}

function isAppUrl(url: string): boolean {
  return /^(tauri:|https?:\/\/(tauri\.localhost|localhost|127\.0\.0\.1))/.test(url);
}

describe('External link navigation policy', () => {
  before(async () => {
    await waitForApp();
    await resetApp(USER_ID);
    await navigateViaHash('/chat');
    await waitForTestId('root-shell-sidebar');
  });

  it('observes an in-app navigation, so "unchanged" below means something', async () => {
    // The control. Without it, every assertion in this file could pass against
    // a `getUrl()` that had stopped reporting changes at all.
    const before = await currentUrl();
    await navigateViaHash('/settings');
    await browser.waitUntil(async () => (await currentUrl()) !== before, {
      timeout: 15_000,
      timeoutMsg:
        'getUrl() never observed an in-app route change, so it cannot witness a remote one',
    });

    await navigateViaHash('/chat');
    await waitForTestId('root-shell-sidebar');
  });

  it('refuses a scripted remote navigation of the main webview', async () => {
    const before = await currentUrl();
    expect(isAppUrl(before)).toBe(true);

    await browser.execute((url: string) => {
      window.location.href = url;
    }, REMOTE_URL);

    // Give a real navigation time to commit. If the policy were absent this is
    // long enough for the webview to have left the app.
    await browser.pause(4_000);

    // Asserted as a string rather than a boolean so a failure prints the URL
    // the webview actually reached — `expect(false).toBe(true)` would not.
    // WDIO's `expect` takes no message argument.
    const after = await currentUrl();
    expect(isAppUrl(after) ? 'stayed in the app' : `navigated away to ${after}`).toBe(
      'stayed in the app'
    );
    expect(after).not.toContain('openhuman-e2e.invalid');
  });

  it('refuses a remote link click in the main webview', async () => {
    // The scripted case above and this one fail differently: an `href`
    // assignment and a user-gesture link click take different paths into the
    // webview, and a policy installed on only one of them would pass the other.
    const before = await currentUrl();
    expect(isAppUrl(before)).toBe(true);

    await browser.execute((url: string) => {
      const anchor = document.createElement('a');
      anchor.id = 'e2e-external-link-probe';
      anchor.href = url;
      anchor.target = '_self';
      anchor.textContent = 'external probe';
      document.body.appendChild(anchor);
      anchor.click();
    }, REMOTE_URL);

    await browser.pause(4_000);

    const after = await currentUrl();
    expect(isAppUrl(after) ? 'stayed in the app' : `link click navigated to ${after}`).toBe(
      'stayed in the app'
    );

    // The app must still be the app: a cancelled navigation should leave the
    // renderer untouched, not tear its tree down.
    await waitForTestId('root-shell-sidebar');

    await browser.execute(() => {
      document.getElementById('e2e-external-link-probe')?.remove();
    });
  });
});
