/**
 * The conversation map rail — assistant-ui's `conversation-map`, on the real
 * chat path.
 *
 * `conversation-map` landed in #6604 with no end-to-end coverage. The unit test
 * (`features/conversations/aui/ChatConversationMap.test.tsx`) renders it against
 * a mocked runtime with a fixed four-message fixture, so it proves the element
 * draws ticks for *that* fixture. Two things it cannot reach: that the rail
 * groups a real thread into turns rather than counting messages, and that
 * selecting a tick moves the transcript — `onSelect` scrolls a real viewport and
 * jsdom has no layout.
 *
 * Product path, read rather than assumed:
 *
 *   AssistantUiChat                     (features/conversations/components)
 *     └─ ChatConversationMap            data-testid=chat-conversation-map
 *          └─ Thread components={{ ConversationMap: ChatConversationMapRail }}
 *               └─ thread.tsx:426 renders the slot inside the scrolling viewport
 *                    └─ ConversationMapAui side="right"
 *                         └─ nav[data-slot="conversation-map"]
 *                              └─ button[data-slot="conversation-map-tick"] per turn
 *
 * Deliberately NOT the dev gallery at `/dev/tools`, which imports the same
 * element and would prove nothing about the product.
 *
 * # Why this seeds thread messages instead of running agent turns
 *
 * `conversation-map` is a presentation component over `state.thread.messages`.
 * Producing those messages by running real turns made the spec depend on turn
 * lifecycle rather than on the rail, and every settle signal available was
 * either vacuous or measured the wrong thing. Measured across several runs, not
 * assumed:
 *
 *  - `stop-generation-button` belongs to the composer, and a turn driven over
 *    `channel_web_chat` need not put the composer into its generating state, so
 *    `toBeHidden` on it passes instantly.
 *  - A reply's FIRST streamed chunk renders ~20ms into the stream, so the next
 *    turn goes out mid-turn. The default `queue_mode` is `interrupt` and the
 *    superseded turn is discarded: after two turns the store held
 *    `messages=2[user,agent]` and the rail correctly drew ONE tick.
 *  - A reply's LAST chunk renders before the turn's terminal event, so the same
 *    supersede happens in a narrower window.
 *  - `queue_mode: 'followup'` did not accumulate either.
 *
 * `threads_message_append` is the RPC the renderer itself posts
 * (`services/api/threadApi.ts:85`), and `loadThreadMessages` is the documented
 * rehydration path, so seeding this way fills the same store the live path
 * fills — without making a rail test a turn-lifecycle test. No mock LLM is
 * scripted here because none is needed.
 */
import { expect, type Locator, type Page, test } from '@playwright/test';

import { callRpcFromPage, waitForSelectedThreadId } from '../helpers/chat-drive';
import { bootAuthenticatedPage, dismissWalkthroughIfPresent } from '../helpers/core-rpc';

const USER_ID = 'pw-chat-conversation-map';

/**
 * One prompt per turn, and each is its turn's whole first line — so it is also
 * the tick's `aria-label` verbatim. `describe()` in `conversation-map.aui.tsx`
 * takes the head message's first line and cuts at a word boundary past 72
 * chars; all three are well under that, so no cut applies.
 */
const PROMPTS = [
  'MAPTURN-ONE where does the deploy config live',
  'MAPTURN-TWO which lane runs the rust e2e suite',
  'MAPTURN-THREE how long is the approval park window',
] as const;

/** Long enough that three turns overflow the viewport, so selection can scroll. */
const agentReply = (index: number) =>
  `Reply to turn ${index}. ` +
  'Padding so the transcript overflows its viewport and the selection test has somewhere to scroll from. '.repeat(
    6
  );

const rail = (page: Page): Locator => page.locator('nav[data-slot="conversation-map"]');
const ticks = (page: Page): Locator => page.locator('button[data-slot="conversation-map-tick"]');
const viewport = (page: Page): Locator => page.locator('[data-slot="aui_thread-viewport"]');

async function openChat(page: Page): Promise<void> {
  await bootAuthenticatedPage(page, USER_ID, '/chat');
  await dismissWalkthroughIfPresent(page);
  await expect(page.getByTestId('chat-message-input')).toBeVisible({ timeout: 30_000 });
}

async function appendMessage(
  page: Page,
  threadId: string,
  id: string,
  sender: 'user' | 'agent',
  content: string,
  createdAt: string
): Promise<void> {
  await callRpcFromPage(page, 'openhuman.threads_message_append', {
    thread_id: threadId,
    message: { id, content, type: 'text', extraMetadata: {}, sender, createdAt },
  });
}

/** Append one user+agent pair per prompt, oldest first. */
async function seedTurns(page: Page, threadId: string): Promise<void> {
  for (const [index, prompt] of PROMPTS.entries()) {
    const at = (offset: number) =>
      new Date(Date.UTC(2026, 0, 1, 0, index * 2, offset)).toISOString();
    await appendMessage(page, threadId, `map-user-${index}`, 'user', prompt, at(0));
    await appendMessage(page, threadId, `map-agent-${index}`, 'agent', agentReply(index), at(1));
  }
}

/** Reload so the UI rehydrates the seeded thread, and wait for it to land. */
async function reloadWithSeededThread(page: Page): Promise<void> {
  await page.reload();
  await dismissWalkthroughIfPresent(page);
  await expect(page.getByTestId('chat-message-input')).toBeVisible({ timeout: 30_000 });
  await expect
    .poll(async () => page.locator('[data-message-id]').count(), {
      timeout: 30_000,
      message: 'the seeded thread never rehydrated after the reload',
    })
    .toBe(PROMPTS.length * 2);
}

/** Scroll offset of the transcript viewport. */
async function scrollTop(page: Page): Promise<number> {
  return page.evaluate(() => {
    const el = document.querySelector<HTMLElement>('[data-slot="aui_thread-viewport"]');
    return el?.scrollTop ?? -1;
  });
}

test.describe.configure({ timeout: 180_000 });

test.describe('Conversation map rail', () => {
  test('draws one tick per turn, not one per message', async ({ page }) => {
    await openChat(page);
    const threadId = await waitForSelectedThreadId(page);

    await expect(page.getByTestId('chat-conversation-map')).toBeVisible();
    // An unseeded thread draws nothing. Without this control the count below
    // would also pass for a rail that had been showing three ticks all along.
    await expect(ticks(page)).toHaveCount(0);

    await seedTurns(page, threadId);
    await reloadWithSeededThread(page);

    // Six messages, three turns. This is the claim the unit test cannot make:
    // a rail that drew a tick per MESSAGE would show 6 and still look plausible
    // against a single-turn fixture.
    await expect(ticks(page)).toHaveCount(PROMPTS.length, { timeout: 30_000 });
    await expect(rail(page)).toBeVisible();

    // Each tick carries its own turn's user message, in order — so a rail that
    // drew the right NUMBER of ticks from the wrong messages (the agent
    // replies, say) passes the count check and fails this one.
    const labels = await ticks(page).evaluateAll(nodes =>
      nodes.map(node => node.getAttribute('aria-label') ?? '')
    );
    expect(labels).toEqual([...PROMPTS]);

    // Exactly one tick is the one being read.
    await expect(
      page.locator('button[data-slot="conversation-map-tick"][data-active]')
    ).toHaveCount(1);
  });

  test('selecting a tick moves the transcript to that turn', async ({ page }) => {
    await openChat(page);
    const threadId = await waitForSelectedThreadId(page);

    await seedTurns(page, threadId);
    await reloadWithSeededThread(page);
    await expect(ticks(page)).toHaveCount(PROMPTS.length, { timeout: 30_000 });

    // Land at the bottom, where the newest turn is, so selecting the first turn
    // has somewhere to travel from.
    await viewport(page).evaluate(el => el.scrollTo({ top: el.scrollHeight }));
    await expect.poll(async () => scrollTop(page), { timeout: 10_000 }).toBeGreaterThan(0);
    const bottom = await scrollTop(page);

    // Non-vacuity control: `select()` is a no-op on a viewport that cannot
    // scroll, so without a scrollable transcript the assertion below would pass
    // against a broken handler.
    expect(bottom).toBeGreaterThan(0);

    await ticks(page).first().click();

    // `scrollTo({ behavior: 'smooth' })` animates, so poll rather than read
    // once. The claim is that the transcript moved back toward the first turn,
    // not that it reached an exact offset — pinning the offset would make this
    // a layout test.
    await expect
      .poll(async () => scrollTop(page), {
        timeout: 15_000,
        message:
          'selecting the first tick must scroll the transcript back toward that turn; ' +
          'the viewport never moved from the bottom',
      })
      .toBeLessThan(bottom);

    // And it moved to the right place: the first turn's prompt is on screen.
    await expect(page.getByText(PROMPTS[0], { exact: false }).first()).toBeInViewport({
      timeout: 15_000,
    });
  });
});
