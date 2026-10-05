// @ts-nocheck
/**
 * E2E: Onboarding — Simple (Cloud) vs Advanced (Custom) modes.
 *
 * Verifies:
 *   - Phase A — Simple/Cloud path: fresh login → Welcome → Runtime choice
 *     (Cloud) → /home. `onboarding_completed = true` lands in
 *     `${OPENHUMAN_WORKSPACE}/config.toml` immediately.
 *
 *   - Phase B — Advanced/Custom path (Default on every wizard step):
 *     reset onboarding flag → Welcome → Runtime choice (Custom) →
 *     Inference (Default) → Search (Default) → Vault (Finish).
 *     The custom wizard is exactly three steps; Voice, OAuth and Embeddings
 *     are no longer part of it (they stay configurable from Settings).
 *     Asserts each of the three step containers renders with the expected
 *     `data-testid`, that the stepper shows Inference / Search / Vault and
 *     that the retired steps are never mounted.
 *
 *   - Phase C — Advanced/Custom path using "Skip for now" on the optional
 *     Search step: Inference (Default) → Search (skip, no key configured) →
 *     Vault (Finish), and `onboarding_completed = true` lands in config.toml.
 *
 * Auth is the bypass deep-link path. The mock API server runs on the same
 * port the dist bundle was built against (see `app/scripts/e2e-run-session.sh`).
 * No real network is touched.
 */
import { waitForAppReady, waitForAuthBootstrap } from '../helpers/app-helpers';
import { readBool, readConfigToml, topLevelValue } from '../helpers/config-toml';
import { callOpenhumanRpc } from '../helpers/core-rpc';
import { triggerAuthDeepLinkBypass } from '../helpers/deep-link-helpers';
import { waitForWebView, waitForWindowVisible } from '../helpers/element-helpers';
import { resetApp } from '../helpers/reset-app';
import { dismissBootCheckGateIfVisible } from '../helpers/shared-flows';
import {
  resetMockBehavior,
  setMockBehavior,
  startMockServer,
  stopMockServer,
} from '../mock-server';

const STEP_LOG_PREFIX = '[onboarding-modes]';

function stepLog(message: string): void {
  console.log(`${STEP_LOG_PREFIX} ${message}`);
}

async function pause(ms: number): Promise<void> {
  await browser.pause(ms);
}

/**
 * Click a button by `data-testid`. Returns true if the click landed.
 */
async function clickTestId(testId: string, timeout = 10_000): Promise<boolean> {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const status = await browser.execute(id => {
      const el = document.querySelector<HTMLElement>(`[data-testid="${id}"]`);
      if (!el) return 'missing';
      if ((el as HTMLButtonElement).disabled) return 'disabled';
      // Ensure the element is visible and has layout before clicking.
      const rect = el.getBoundingClientRect();
      if (rect.width === 0 || rect.height === 0) return 'no-layout';
      ['mousedown', 'mouseup', 'click'].forEach(type => {
        el.dispatchEvent(
          new MouseEvent(type, { bubbles: true, cancelable: true, view: window, button: 0 })
        );
      });
      return 'clicked';
    }, testId);
    if (status === 'clicked') return true;
    await pause(400);
  }
  return false;
}

async function testIdExists(testId: string, timeout = 10_000): Promise<boolean> {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const found = await browser.execute(
      id => document.querySelector(`[data-testid="${id}"]`) !== null,
      testId
    );
    if (found) return true;
    await pause(400);
  }
  return false;
}

async function currentHash(): Promise<string> {
  return browser.execute(() => window.location.hash || '');
}

async function waitForHash(prefix: string, timeout = 15_000): Promise<boolean> {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const hash = await currentHash();
    if (hash.startsWith(prefix)) return true;
    await pause(400);
  }
  return false;
}

async function resetOnboardingFlagAndReload(): Promise<void> {
  stepLog('Resetting onboarding_completed=false via RPC');
  const res = await callOpenhumanRpc<{ completed: boolean }>(
    'openhuman.config_set_onboarding_completed',
    { value: false }
  );
  if (!res.ok) {
    throw new Error(`config.set_onboarding_completed failed: ${JSON.stringify(res)}`);
  }
  await browser.execute(() => {
    try {
      window.localStorage.clear();
      window.sessionStorage.clear();
    } catch {
      /* ignore */
    }
    window.location.replace('#/');
    window.location.reload();
  });
  await waitForWindowVisible(25_000);
  await waitForWebView(15_000);
  await waitForAppReady(15_000);
  await dismissBootCheckGateIfVisible(8_000);
  await triggerAuthDeepLinkBypass('e2e-onboarding-modes');
  await waitForAuthBootstrap(15_000);
  await dismissBootCheckGateIfVisible(8_000);
  // Auth bootstrap restores the server-side onboarding snapshot, which can
  // overwrite the pre-auth test flag. Apply the requested state once auth is
  // settled, then reload so the onboarding gate consumes the new snapshot.
  const postAuthRes = await callOpenhumanRpc<{ completed: boolean }>(
    'openhuman.config_set_onboarding_completed',
    { value: false }
  );
  if (!postAuthRes.ok) {
    throw new Error(
      `post-auth config.set_onboarding_completed failed: ${JSON.stringify(postAuthRes)}`
    );
  }
  await browser.execute(() => window.location.reload());
  await waitForAppReady(15_000);
  await dismissBootCheckGateIfVisible(8_000);
  // Wait for the welcome step to mount before returning.
  const onWelcome = await waitForHash('#/onboarding', 15_000);
  if (!onWelcome) {
    stepLog(`hash after reset = ${await currentHash()}`);
    throw new Error('onboarding overlay did not re-mount after flag reset');
  }
}

async function clickOnboardingNext(): Promise<void> {
  // The Welcome step button is the same shared `onboarding-next-button`.
  const ok = await clickTestId('onboarding-next-button', 10_000);
  if (!ok) {
    throw new Error('onboarding-next-button missing or stayed disabled');
  }
}

async function waitForCustomSelection(timeout = 5_000): Promise<boolean> {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const selected = await browser.execute(() => {
      const el = document.querySelector('[data-testid="onboarding-runtime-choice-custom"]');
      return el?.getAttribute('aria-pressed') === 'true';
    });
    if (selected) return true;
    await pause(250);
  }
  return false;
}

/** Rendered stepper labels of the custom wizard, in DOM order. */
async function stepperLabels(): Promise<string[]> {
  return browser.execute(() => {
    const items = document.querySelectorAll('[data-testid="onboarding-wizard-stepper"] > li');
    return Array.from(items).map(li =>
      (li.querySelector('span:last-child')?.textContent ?? '').trim()
    );
  });
}

async function advanceFromWelcomeToCustomInference(phase: string): Promise<void> {
  const reachedChoiceOrInference = await browser
    .waitUntil(
      async () => {
        if (await testIdExists('onboarding-runtime-choice-step', 250)) return true;
        if (await testIdExists('onboarding-custom-inference-step', 250)) return true;
        if (await testIdExists('onboarding-welcome-step', 250)) {
          await clickOnboardingNext();
        }
        return false;
      },
      {
        timeout: 15_000,
        interval: 300,
        timeoutMsg: `${phase}: neither runtime choice nor custom inference mounted`,
      }
    )
    .catch(() => false);
  if (!reachedChoiceOrInference) {
    stepLog(`${phase}: hash while waiting for runtime/custom inference = ${await currentHash()}`);
  }
  expect(reachedChoiceOrInference).toBe(true);

  if (await testIdExists('onboarding-runtime-choice-step', 500)) {
    const inferenceReached = await browser
      .waitUntil(
        async () => {
          if (await testIdExists('onboarding-custom-inference-step', 250)) return true;
          if (!(await testIdExists('onboarding-runtime-choice-step', 250))) return false;

          if (!(await clickTestId('onboarding-runtime-choice-custom', 2_000))) {
            return false;
          }
          const customSelected = await waitForCustomSelection(2_000);
          if (!customSelected) {
            stepLog(`${phase}: Custom card click did not register — retrying`);
            return false;
          }

          await clickOnboardingNext();
          return await testIdExists('onboarding-custom-inference-step', 1_000);
        },
        {
          timeout: 15_000,
          interval: 500,
          timeoutMsg: `${phase}: custom inference did not mount after runtime choice`,
        }
      )
      .catch(() => false);
    expect(inferenceReached).toBe(true);
  }

  const inferenceVisible = await testIdExists('onboarding-custom-inference-step', 15_000);
  if (!inferenceVisible) {
    stepLog(`${phase}: hash before custom inference assertion = ${await currentHash()}`);
  }
  expect(inferenceVisible).toBe(true);
}

async function waitForHome(timeout = 20_000): Promise<boolean> {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const hash = await currentHash();
    // Home was merged into the unified chat surface: /home redirects to /chat
    // (AppRoutes.tsx). Accept either so the wizard's post-finish landing check
    // survives the IA change.
    if (hash.startsWith('#/home') || hash.startsWith('#/chat')) return true;
    await pause(400);
  }
  return false;
}

describe('Onboarding modes — Simple (Cloud) vs Advanced (Custom)', function () {
  this.timeout(90_000);

  before(async function beforeSuite() {
    // Reset + auth + onboarding bootstrap can exceed the default 30s hook budget.
    this.timeout(90_000);
    await startMockServer();
    resetMockBehavior();
    setMockBehavior('composioConnections', '[]');
    // Reset state but skip the built-in onboarding walker — we walk it
    // ourselves to assert the per-step UI.
    // This spec needs a non-local authenticated session to exercise the
    // Cloud choice. test_reset alone leaves a prior app-session profile in
    // place; that profile can be local and intentionally redirects straight
    // to the Custom wizard, bypassing the runtime-choice page altogether.
    await resetApp('e2e-onboarding-modes', { skipAuth: true, clearAuthSession: true });
    // resetApp restores onboarding_completed=true for normal specs. Auth
    // bootstrap refreshes the renderer snapshot, so change the flag *after*
    // login; doing it before auth lets the stale "complete" snapshot bounce
    // the runtime-choice route to chat.
    await triggerAuthDeepLinkBypass('e2e-onboarding-modes');
    await waitForAuthBootstrap(15_000);
    await dismissBootCheckGateIfVisible(8_000);
    stepLog('Setting onboarding_completed=false after auth bootstrap');
    await callOpenhumanRpc('openhuman.config_set_onboarding_completed', { value: false });
    await browser.execute(() => {
      window.location.replace('#/onboarding/welcome');
      window.location.reload();
    });
    await waitForWindowVisible(25_000);
    await waitForWebView(15_000);
    await waitForAppReady(15_000);
    await waitForAuthBootstrap(15_000);
    await waitForHash('#/onboarding', 15_000);
  });

  after(async () => {
    resetMockBehavior();
    await stopMockServer();
  });

  // ───────────────────────────────────────────────────────────────────────
  // Phase A — Simple (Cloud)
  // ───────────────────────────────────────────────────────────────────────

  it('simple/cloud path: welcome → runtime-choice → cloud → home', async () => {
    // Step 0 — Welcome screen.
    const welcomeVisible = await testIdExists('onboarding-next-button', 15_000);
    expect(welcomeVisible).toBe(true);
    await clickOnboardingNext();

    // Step 1 — Runtime choice. The card is preselected to Cloud, so simply
    // clicking the next button continues the cloud path.
    // The Windows CEF runner can take more than 10 seconds to commit the
    // route transition after a cold auth/onboarding bootstrap.
    const choiceVisible = await testIdExists('onboarding-runtime-choice-step', 20_000);
    expect(choiceVisible).toBe(true);
    const cloudCardVisible = await testIdExists('onboarding-runtime-choice-cloud', 5_000);
    expect(cloudCardVisible).toBe(true);
    // Explicitly click the Cloud card so the test is robust against the
    // default selection changing in the future.
    await clickTestId('onboarding-runtime-choice-cloud');
    await pause(500);
    await clickOnboardingNext();

    const landed = await waitForHome(20_000);
    if (!landed) stepLog(`current hash after cloud finish: ${await currentHash()}`);
    expect(landed).toBe(true);
  });

  it('simple/cloud path: config.toml reflects onboarding_completed=true', async () => {
    // The setOnboardingCompletedFlag RPC writes config.save() before the
    // navigate() in OnboardingLayout, but I/O can lag a tick. Poll briefly.
    let value: boolean | null = null;
    const deadline = Date.now() + 8_000;
    while (Date.now() < deadline) {
      value = readBool(topLevelValue(readConfigToml(), 'onboarding_completed'));
      if (value === true) break;
      await pause(400);
    }
    if (value !== true) {
      stepLog(`config.toml head:\n${readConfigToml().split('\n').slice(0, 30).join('\n')}`);
    }
    expect(value).toBe(true);
  });

  // ───────────────────────────────────────────────────────────────────────
  // Phase B — Advanced (Custom), Default on every step
  // ───────────────────────────────────────────────────────────────────────

  it('advanced/custom path: walks the three wizard steps with Default choice', async function () {
    // resetOnboardingFlagAndReload includes waitForWindowVisible(25_000), needs extra budget.
    this.timeout(90_000);
    await resetOnboardingFlagAndReload();

    await advanceFromWelcomeToCustomInference('Phase B');

    // Step 1 of 3 — Custom Inference (Default). The stepper must list exactly
    // the three live steps, in order.
    expect(await testIdExists('onboarding-custom-inference-step', 10_000)).toBe(true);
    expect(await stepperLabels()).toEqual(['Inference', 'Search', 'Vault']);
    expect(await clickTestId('onboarding-custom-inference-step-default')).toBe(true);
    await pause(400);
    await clickOnboardingNext();

    // Step 2 of 3 — Custom Search (Default).
    expect(await testIdExists('onboarding-custom-search-step', 10_000)).toBe(true);
    expect(await stepperLabels()).toEqual(['Inference', 'Search', 'Vault']);
    expect(await clickTestId('onboarding-custom-search-step-default')).toBe(true);
    await pause(400);
    await clickOnboardingNext();

    // Step 3 of 3 — Custom Vault. Final step → Finish. VaultSetupStep hides the
    // choice cards and auto-selects "configure" only for LOCAL sessions
    // (`defaultDisabled={isLocalSession}`); the E2E logs in via the cloud-auth
    // deep link, so the session is non-local — the choice cards ARE shown with
    // an enabled default, and the Finish button stays disabled (`choice === null`)
    // until one is picked. Select the default when the card is present; a local
    // session (cards hidden) just advances.
    expect(await testIdExists('onboarding-custom-vault-step', 10_000)).toBe(true);
    if (await testIdExists('onboarding-custom-vault-step-default', 2_000)) {
      await clickTestId('onboarding-custom-vault-step-default');
    }
    await pause(400);
    await clickOnboardingNext();

    // The retired steps must never have been part of the flow.
    expect(await testIdExists('onboarding-custom-voice-step', 500)).toBe(false);
    expect(await testIdExists('onboarding-custom-oauth-step', 500)).toBe(false);
    expect(await testIdExists('onboarding-custom-embeddings-step', 500)).toBe(false);

    const landed = await waitForHome(20_000);
    if (!landed) stepLog(`current hash after custom finish: ${await currentHash()}`);
    expect(landed).toBe(true);

    // Re-confirm the persisted flag is true after the second completion.
    let value: boolean | null = null;
    const deadline = Date.now() + 8_000;
    while (Date.now() < deadline) {
      value = readBool(topLevelValue(readConfigToml(), 'onboarding_completed'));
      if (value === true) break;
      await pause(400);
    }
    expect(value).toBe(true);
  });

  // ───────────────────────────────────────────────────────────────────────
  // Phase C — Advanced (Custom), skip the optional Search step
  // ───────────────────────────────────────────────────────────────────────

  it('advanced/custom path: Skip for now on Search advances to Vault and completes onboarding', async function () {
    // resetOnboardingFlagAndReload includes waitForWindowVisible(25_000), needs extra budget.
    this.timeout(90_000);
    await resetOnboardingFlagAndReload();

    // Welcome → Runtime choice (Custom) → Inference (Default).
    await advanceFromWelcomeToCustomInference('Phase C');

    expect(await clickTestId('onboarding-custom-inference-step-default')).toBe(true);
    await pause(400);
    await clickOnboardingNext();

    // Search is the one genuinely optional step: the skip control moves on
    // without a key configured, straight to the Vault step.
    expect(await testIdExists('onboarding-custom-search-step', 10_000)).toBe(true);
    expect(await testIdExists('onboarding-search-skip', 5_000)).toBe(true);
    expect(await clickTestId('onboarding-search-skip')).toBe(true);

    expect(await testIdExists('onboarding-custom-vault-step', 10_000)).toBe(true);
    if (await testIdExists('onboarding-custom-vault-step-default', 2_000)) {
      await clickTestId('onboarding-custom-vault-step-default');
    }
    await pause(400);
    await clickOnboardingNext();

    expect(await waitForHome(20_000)).toBe(true);

    let value: boolean | null = null;
    const deadline = Date.now() + 8_000;
    while (Date.now() < deadline) {
      value = readBool(topLevelValue(readConfigToml(), 'onboarding_completed'));
      if (value === true) break;
      await pause(400);
    }
    expect(value).toBe(true);
  });
});
