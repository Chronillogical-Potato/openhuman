/**
 * The running-status line appears while a turn is in flight and is GONE once
 * it settles — `elements/agent-status` family, through its real product path.
 *
 * Path under test, end to end:
 *
 *   `AssistantUiChat.tsx:316`  installs `AgentRunningStatus` as the thread's
 *                              `RunningStatus` slot component;
 *   `thread.tsx:844`           `RunningStatusSlot` renders it inside
 *                              `<AuiIf condition={s => s.thread.isRunning}>`;
 *   `aui/AgentRunningStatus.tsx:58` with no registered tasks that is a
 *                              `GenerationLoader` carrying
 *                              `data-testid="agent-running-status-thinking"`.
 *
 * This is deliberately NOT a "the element renders" spec. Every piece above can
 * pass its own unit test while the composed surface is wrong in the one way
 * that matters to a user: a status line that never clears. `isRunning` going
 * stale leaves a permanent "Thinking…" under a finished answer, and nothing
 * else on the page contradicts it — the answer is there, so the app looks
 * busy forever with no error to report. A mounted-component test cannot see
 * that, because it never owns the transition.
 *
 * So all three phases are asserted, in order: absent before, present during,
 * absent after. The before-assertion is what makes the after-assertion mean
 * something — "still absent" is not evidence of clearing.
 *
 * Turns go over `openhuman.channel_web_chat` (`helpers/chat-drive.ts`), never
 * the composer.
 *
 * ---------------------------------------------------------------------------
 * If you are about to revert-prove this spec, read this first.
 *
 * The obvious fault is to drop the `isRunning` gate on `RunningStatusSlot`
 * (`thread.tsx:848` → `condition={() => true}`). It does turn both tests red,
 * so it proves the spec is not vacuous — but it proves the WRONG HALF. With
 * the gate gone the line renders always, including before a turn, so phase 1
 * ("absent before") fires first and the run never reaches phase 3, the
 * clears-once-it-settles assertion this spec exists for. A red at phase 1
 * tells you nothing about whether phase 3 can see anything at all.
 *
 * The fault that proves phase 3 is one that leaves phase 1 green: make
 * `isRunning` STICKY after completion rather than always-true — e.g. have
 * `chatRuntimeSlice`'s turn-completion path stop deleting the thread's
 * `inferenceTurnLifecycleByThread` entry — the `delete` sites are
 * `chatRuntimeSlice.ts:2477`, `:2480`, `:2508` and `:2647`, and a fault needs
 * whichever one the completing turn actually takes, so check before assuming.
 * The line is then correctly absent before, correctly present during, and
 * wrongly present after, which is exactly the shipped regression described
 * above.
 *
 * As of this writing only the first fault has been run. Phase 3 is asserted
 * but not fault-proven.
 * ---------------------------------------------------------------------------
 *
 * NOT covered here: the `TaskTray` branch of the same component
 * (`agent-running-status-tasks`, rendered when `s.thread.tasks` is non-empty).
 * That state is assistant-ui's own, fed only by nested sub-agent transcripts —
 * `AgentRunningStatus.tsx:13-16` says a plain tool call is not a task by that
 * definition — so producing it needs a real delegation fixture rather than a
 * scripted tool call. Written up rather than faked.
 */
import { expect, type Locator, type Page, test } from '@playwright/test';

import {
  resetMock,
  sendTurn,
  setKeywordRules,
  setMockBehavior,
  waitForSelectedThreadId,
} from '../helpers/chat-drive';
import { bootAuthenticatedPage, dismissWalkthroughIfPresent } from '../helpers/core-rpc';

const USER_ID = 'pw-agent-running-status';

const PROMPT = 'RUNSTATUS summarise the changelog';
/**
 * Long enough that the mock streams it as several delayed chunks, so the
 * in-flight window is observable rather than a race. Short enough to stay
 * inside the turn budget.
 */
const ANSWER = [
  'RUNSTATUS-ANSWER-MARKER',
  'the changelog covers the release notes, the migration guide,',
  'the deprecations list and the upgrade steps for the next version.',
].join(' ');
const ANSWER_MARKER = 'RUNSTATUS-ANSWER-MARKER';

const RULES = [{ keyword: 'RUNSTATUS', content: ANSWER }];

const thinkingLine = (page: Page): Locator => page.getByTestId('agent-running-status-thinking');
const stopButton = (page: Page): Locator => page.getByTestId('stop-generation-button');
const answer = (page: Page): Locator => page.getByText(ANSWER_MARKER, { exact: false });

async function openChat(page: Page): Promise<void> {
  await bootAuthenticatedPage(page, USER_ID, '/chat');
  await dismissWalkthroughIfPresent(page);
  await expect(page.getByTestId('chat-message-input')).toBeVisible({ timeout: 30_000 });
}

// A browser spec that waits on a real streamed turn; same budget as the other
// chat specs in this directory.
test.describe.configure({ timeout: 120_000 });

test.describe('Agent running status', () => {
  test.beforeEach(async () => {
    await resetMock();
    await setKeywordRules(RULES);
    // Slow the stream so "in flight" is a window, not an instant. Without
    // this the mock answers in one chunk and the running phase can close
    // before the first poll, which would make the present-during assertion
    // flaky rather than wrong.
    await setMockBehavior('llmStreamChunkDelayMs', '300');
  });

  test('appears while the turn runs and clears once it settles', async ({ page }) => {
    await openChat(page);

    // Phase 1 — absent before. Without this the phase-3 assertion would be
    // satisfied by a line that never rendered at all.
    await expect(
      thinkingLine(page),
      'the running status must not be on screen before any turn is sent'
    ).toHaveCount(0);

    const threadId = await waitForSelectedThreadId(page);
    await sendTurn(page, threadId, PROMPT);

    // Phase 2, staged upstream-first so a red says WHICH layer broke.
    //
    // `stop-generation-button` and the thinking line are gated on the same
    // fact — `chatRuntime.inferenceTurnLifecycleByThread` becoming
    // `started`/`streaming`, which is what `useOpenHumanExternalStore.ts:404`
    // turns into `s.thread.isRunning` for `RunningStatusSlot`
    // (`thread.tsx:848`). The stop button is the cheaper, older signal, so it
    // goes first: if BOTH are missing the turn never registered as running at
    // all and nothing about `agent-status` has been tested; if only the
    // thinking line is missing, the lifecycle was set and the status slot is
    // the thing at fault. Asserting the element first would have collapsed
    // those two very different findings into one "element(s) not found".
    await expect(
      stopButton(page),
      'the runtime never reported a running turn, so the running-status slot was never under test'
    ).toBeVisible({ timeout: 60_000 });
    await expect(
      thinkingLine(page),
      'the runtime reported a running turn but no running status rendered for it'
    ).toBeVisible({ timeout: 30_000 });

    // The turn genuinely completes — the answer lands.
    await expect(answer(page), 'the scripted answer never reached the browser').toBeVisible({
      timeout: 60_000,
    });

    // Phase 3 — gone after. This is the assertion the whole spec exists for.
    await expect(
      thinkingLine(page),
      'the running status survived the turn it was reporting on'
    ).toHaveCount(0, { timeout: 30_000 });
    await expect(
      stopButton(page),
      'the stop button survived the turn it was reporting on'
    ).toHaveCount(0, { timeout: 30_000 });
  });

  test('stays cleared after the turn settles, rather than flickering back', async ({ page }) => {
    await openChat(page);

    const threadId = await waitForSelectedThreadId(page);
    await sendTurn(page, threadId, PROMPT);
    await expect(answer(page)).toBeVisible({ timeout: 60_000 });
    await expect(thinkingLine(page)).toHaveCount(0, { timeout: 30_000 });

    // A late socket frame re-arming `isRunning` after the answer is the shape
    // this catches: the first spec's poll would have already passed, so a
    // re-appearance a second later would go unnoticed. Hold the assertion
    // across a window instead of sampling once.
    const settled = Date.now() + 5_000;
    while (Date.now() < settled) {
      await expect(
        thinkingLine(page),
        'the running status came back after the turn had already settled'
      ).toHaveCount(0);
      await page.waitForTimeout(500);
    }
  });
});
