/**
 * Find-in-conversation — assistant-ui's `conversation-search`, on the real
 * chat path.
 *
 * Product path, read rather than assumed:
 *
 *   AssistantUiChat
 *     └─ ChatConversationMap            data-testid=chat-conversation-map
 *          ├─ Cmd/Ctrl+F while focus is inside the container
 *          └─ ConversationSearch        data-testid=chat-conversation-search
 *
 * The unit test (`ChatConversationMap.test.tsx`) opens the bar with Ctrl+F and
 * checks the counter reads `1/2` for a two-message fixture. What it cannot
 * reach is the cycle this spec covers — a real thread, a hit count that tracks
 * what was actually said, stepping between matches, and the bar returning to
 * its empty state when the query is cleared. `buildHits` also reads message
 * geometry out of the live viewport for each hit's `position`, which jsdom
 * cannot produce.
 *
 * # The needle appears only in prompts, on purpose
 *
 * `buildHits` searches every message in `thread.messages`, user and assistant
 * alike. If the scripted reply could contain the needle the expected count
 * would depend on the mock's output as well as the prompts, and a wrong count
 * would be ambiguous between the two. The replies here are fixed text with no
 * overlap, so the expected hit count is exactly the number of prompts sent.
 *
 * # What this does NOT assert, and why
 *
 * The brief for this work described search as narrowing the conversation-map
 * entries and restoring them when cleared. It does not, and the test says so
 * rather than asserting a behaviour that is not implemented:
 * `ConversationMapAui` takes only `side` and `className`
 * (`conversation-map.aui.tsx:126-132`) and reads `state.thread.messages`
 * directly — the query lives in `ChatConversationMap`'s own state and never
 * reaches it. What narrows is the hit set the search bar owns. The final
 * assertion below pins the rail as *unchanged* across the search, so if the
 * two are ever wired together this spec fails and is updated deliberately
 * instead of silently continuing to describe the old behaviour.
 */
import { expect, type Locator, type Page, test } from '@playwright/test';

import { callRpcFromPage, waitForSelectedThreadId } from '../helpers/chat-drive';
import { bootAuthenticatedPage, dismissWalkthroughIfPresent } from '../helpers/core-rpc';

const USER_ID = 'pw-chat-conversation-search';

/**
 * A token that appears once per prompt and nowhere in the replies, so the
 * expected hit count is the prompt count and nothing else.
 */
const NEEDLE = 'zebrafinch';

const PROMPTS = [
  `SEARCHTURN-ONE the ${NEEDLE} deploy note`,
  `SEARCHTURN-TWO another ${NEEDLE} reference`,
  `SEARCHTURN-THREE a third ${NEEDLE} mention`,
] as const;

/**
 * Replies deliberately share no token with NEEDLE, so the expected hit count is
 * the prompt count and nothing else.
 */
const agentReply = (index: number) => `Answer ${index} with no searchable token in it.`;

const searchBar = (page: Page): Locator => page.getByTestId('chat-conversation-search');
const ticks = (page: Page): Locator => page.locator('button[data-slot="conversation-map-tick"]');

/** The `i/N` (or `0`) counter the element renders beside the input. */
async function counter(page: Page): Promise<string> {
  return searchBar(page)
    .locator('span.tabular-nums')
    .first()
    .innerText()
    .then(text => text.trim());
}

async function openChat(page: Page): Promise<void> {
  await bootAuthenticatedPage(page, USER_ID, '/chat');
  await dismissWalkthroughIfPresent(page);
  await expect(page.getByTestId('chat-message-input')).toBeVisible({ timeout: 30_000 });
}

/**
 * Open the find bar.
 *
 * `useFindShortcut` only fires while `document.activeElement` is inside the
 * container (`ChatConversationMap.tsx`), which is why the container is focused
 * first — it carries `tabIndex={-1}` for exactly this.
 */
async function openFindBar(page: Page): Promise<void> {
  const container = page.getByTestId('chat-conversation-map');
  await container.evaluate(el => (el as HTMLElement).focus());

  // Assert the precondition rather than assume it. `useFindShortcut` ignores
  // the keystroke when focus is outside the container, so without this a
  // focus that never landed reports as "the search bar never opened" and
  // sends the next reader looking at the wrong component.
  await expect
    .poll(
      async () =>
        container.evaluate(
          el => el.contains(document.activeElement) || el === document.activeElement
        ),
      {
        timeout: 5_000,
        message: 'focus never landed inside chat-conversation-map, so Ctrl+F could not be in scope',
      }
    )
    .toBe(true);

  await page.keyboard.press('Control+f');
  await expect(searchBar(page)).toBeVisible({ timeout: 10_000 });
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

/**
 * Seed one user+agent pair per prompt, then reload so the UI rehydrates them.
 *
 * Direct append rather than real agent turns, for the reasons set out at length
 * in `chat-conversation-search`'s sibling `chat-conversation-map.spec.ts`: a
 * search test should not be able to fail because of turn queue semantics.
 */
async function seedThread(page: Page, threadId: string): Promise<void> {
  for (const [index, prompt] of PROMPTS.entries()) {
    const at = (offset: number) =>
      new Date(Date.UTC(2026, 0, 1, 0, index * 2, offset)).toISOString();
    await appendMessage(page, threadId, `search-user-${index}`, 'user', prompt, at(0));
    await appendMessage(page, threadId, `search-agent-${index}`, 'agent', agentReply(index), at(1));
  }
  await page.reload();
  await dismissWalkthroughIfPresent(page);
  await expect(page.getByTestId('chat-message-input')).toBeVisible({ timeout: 30_000 });
  await expect
    .poll(async () => page.locator('[data-message-id]').count(), {
      timeout: 30_000,
      message: 'the seeded thread never rehydrated after the reload',
    })
    .toBe(PROMPTS.length * 2);
  await expect(ticks(page)).toHaveCount(PROMPTS.length, { timeout: 30_000 });
}

test.describe.configure({ timeout: 180_000 });

test.describe('Find in conversation', () => {
  test('narrows to the matches in the thread and clears back to none', async ({ page }) => {
    await openChat(page);
    await seedThread(page, await waitForSelectedThreadId(page));

    // Closed until asked for. Without this the "opens on Ctrl+F" claim below
    // would pass against a bar that was always mounted.
    await expect(searchBar(page)).toHaveCount(0);

    await openFindBar(page);

    // Open but empty: no query, so no hits, and the element renders `0`
    // rather than a ratio.
    expect(await counter(page)).toBe('0');

    const input = searchBar(page).locator('input');
    await input.fill(NEEDLE);

    // One hit per prompt — the count is a claim about the thread's content,
    // not a constant. `1/3` also proves the active index starts at the first
    // match rather than at whatever the previous query left behind.
    await expect
      .poll(async () => counter(page), {
        timeout: 15_000,
        message: `searching for "${NEEDLE}" must find one match per prompt (${PROMPTS.length})`,
      })
      .toBe(`1/${PROMPTS.length}`);

    // The active match is shown in context, with the matched text separated
    // from its surroundings — that split is the element's whole job.
    await expect(searchBar(page).getByText(NEEDLE, { exact: true }).first()).toBeVisible();

    // Stepping forward moves the active index without changing the hit set.
    await searchBar(page).getByRole('button', { name: /next/i }).click();
    await expect.poll(async () => counter(page), { timeout: 10_000 }).toBe(`2/${PROMPTS.length}`);

    // A query that matches nothing narrows to zero rather than leaving the
    // previous results on screen.
    await input.fill('definitely-not-in-this-thread');
    await expect.poll(async () => counter(page), { timeout: 10_000 }).toBe('0');

    // Clearing restores the empty state: back to `0`, and the context row for
    // the active match is gone.
    await input.fill('');
    await expect.poll(async () => counter(page), { timeout: 10_000 }).toBe('0');
    await expect(searchBar(page).getByText(NEEDLE, { exact: true })).toHaveCount(0);

    // Re-querying finds the same matches again, so clearing reset the query
    // and not the underlying hit source.
    await input.fill(NEEDLE);
    await expect.poll(async () => counter(page), { timeout: 10_000 }).toBe(`1/${PROMPTS.length}`);
  });

  test('searching does not disturb the conversation map rail', async ({ page }) => {
    await openChat(page);
    await seedThread(page, await waitForSelectedThreadId(page));

    const before = await ticks(page).count();
    expect(before).toBe(PROMPTS.length);

    await openFindBar(page);
    await searchBar(page).locator('input').fill(NEEDLE);
    await expect.poll(async () => counter(page), { timeout: 15_000 }).toBe(`1/${PROMPTS.length}`);

    // The rail is fed by `state.thread.messages` and knows nothing about the
    // query. Pinned rather than assumed: this is the assertion that fails if
    // the search is ever wired into the rail, forcing the change to be
    // deliberate. See the header note.
    await expect(ticks(page)).toHaveCount(before);
  });
});
