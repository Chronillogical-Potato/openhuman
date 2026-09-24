/**
 * Two turns in flight at once — across threads, and forked inside one.
 *
 * Matrix 4.2.4 is 🟡 with *"dedicated WD E2E is a follow-up"*. Its RU coverage
 * (`web_chat/web_tests.rs`) drives concurrent same-/cross-thread dispatch and
 * cooperative cancellation; its VU coverage (`chatRuntimeSlice`,
 * `ChatRuntimeProvider`) drives the parallel-lane routing through the slice.
 * Both are real, and neither has two live sockets.
 *
 * This is deliberately NOT in `chat-thread-isolation.spec.ts`, which owns the
 * adjacent surface. That file has exactly one turn in flight in every case —
 * thread B is idle throughout — and a routing bug that keys a stream by "the
 * selected thread" instead of by thread id is invisible with one stream and
 * catastrophic with two: the second turn's tokens land in whichever thread the
 * user happens to be looking at. It also drives its turns by typing, which
 * these do not (see `helpers/chat-drive.ts`).
 *
 * Both cases need per-turn content, so the mock is scripted with keyword rules
 * carrying their own `streamScript` rather than the global `llmStreamScript`
 * (which would hand both turns the same text and make them indistinguishable).
 */
import { expect, type Locator, type Page, test } from '@playwright/test';

import {
  resetMock,
  selectedThreadId,
  sendTurn,
  setKeywordRules,
  startNewThread,
  waitForSelectedThreadId,
} from '../helpers/chat-drive';
import { bootAuthenticatedPage, dismissWalkthroughIfPresent } from '../helpers/core-rpc';

const USER_ID = 'pw-chat-parallel-turns';

const ALPHA_PROMPT = 'ALPHA-TURN please';
const BRAVO_PROMPT = 'BRAVO-TURN please';

/** `safeDelayMs` clamps to 1000ms, so the run length comes from chunk count. */
function slowScript(prefix: string, chunks: number): unknown[] {
  return [
    ...Array.from({ length: chunks }, (_, i) => ({ text: `${prefix}${i} `, delayMs: 1000 })),
    { finish: 'stop' },
  ];
}

const RULES = [
  { keyword: 'ALPHA-TURN', streamScript: slowScript('alpha', 18) },
  { keyword: 'BRAVO-TURN', streamScript: slowScript('bravo', 18) },
];

const stopButton = (page: Page): Locator => page.getByTestId('stop-generation-button');

async function openChat(page: Page): Promise<void> {
  await bootAuthenticatedPage(page, USER_ID, '/chat');
  await dismissWalkthroughIfPresent(page);
  await expect(page.getByTestId('chat-message-input')).toBeVisible({ timeout: 30_000 });
}

/** Whole-transcript text, for contiguity assertions a locator cannot express. */
async function transcriptText(page: Page): Promise<string> {
  return page.evaluate(() => document.querySelector('#root')?.textContent ?? '');
}

test.describe.configure({ timeout: 180_000 });

test.describe('Concurrent turns', () => {
  test.beforeEach(async () => {
    await resetMock();
    await setKeywordRules(RULES);
  });

  test('two threads streaming at once keep their own tokens', async ({ page }) => {
    await openChat(page);

    const threadA = await waitForSelectedThreadId(page);
    await sendTurn(page, threadA, ALPHA_PROMPT);
    await expect(page.getByText('alpha0', { exact: false }).last()).toBeVisible({
      timeout: 60_000,
    });

    const threadB = await startNewThread(page);
    expect(threadB, 'the second thread must be a different thread').not.toBe(threadA);
    await sendTurn(page, threadB, BRAVO_PROMPT);

    // Both turns are now genuinely in flight. B is selected, so B's tokens are
    // what this viewport may show — and only B's.
    await expect(page.getByText('bravo0', { exact: false }).last()).toBeVisible({
      timeout: 60_000,
    });
    await expect(
      page.getByText('alpha0', { exact: false }),
      "the other thread's live tokens must not bleed into this one"
    ).toHaveCount(0);
    await expect(stopButton(page), 'B has its own turn to stop').toBeVisible({ timeout: 15_000 });

    // Switch back while BOTH are still running: A must show its own stream,
    // unaffected by the fact that a second turn started after it.
    const rowA = page.getByTestId(`thread-row-${threadA}`);
    await expect(rowA).toBeVisible({ timeout: 15_000 });
    await rowA.click({ force: true });
    await expect.poll(async () => selectedThreadId(page), { timeout: 15_000 }).toBe(threadA);

    await expect(page.getByText('alpha0', { exact: false }).last()).toBeVisible({
      timeout: 20_000,
    });
    await expect(
      page.getByText('bravo0', { exact: false }),
      'a turn started on another thread must never render here'
    ).toHaveCount(0);
  });

  test('two turns forked inside one thread do not interleave into one message', async ({
    page,
  }) => {
    await openChat(page);
    const threadId = await waitForSelectedThreadId(page);

    // `queue_mode: 'parallel'` is the product's own name for this — the other
    // modes ('interrupt' is the default, plus 'steer' / 'followup' / 'collect')
    // would make this a supersede or a queue, not a fork
    // (`web_chat/schemas.rs`, the `queue_mode` input).
    await sendTurn(page, threadId, ALPHA_PROMPT, { queueMode: 'parallel' });
    await expect(page.getByText('alpha0', { exact: false }).last()).toBeVisible({
      timeout: 60_000,
    });
    await sendTurn(page, threadId, BRAVO_PROMPT, { queueMode: 'parallel' });

    await expect(page.getByText('bravo0', { exact: false }).last()).toBeVisible({
      timeout: 60_000,
    });

    // Presence is not the assertion — two lanes folded into one bubble would
    // still contain both strings. Contiguity is: if the lanes interleaved,
    // `alpha0 alpha1 alpha2` could not survive as a run.
    await expect
      .poll(async () => transcriptText(page), {
        timeout: 90_000,
        message: 'the forked lanes interleaved instead of rendering as separate runs',
      })
      .toContain('alpha0 alpha1 alpha2');
    await expect
      .poll(async () => transcriptText(page), { timeout: 90_000 })
      .toContain('bravo0 bravo1 bravo2');
  });
});
