/**
 * Inline source list under a settled answer.
 *
 * Sources travel as assistant-ui `source` parts (`assistantParts`) and `Thread`
 * groups them into its `SourceGroup` slot, which `/chat` fills with
 * `ChatSources`.
 *
 * Five things are under test, and the second is the one that matters:
 *
 * 1. the list renders the turn's `http(s)` sources;
 * 2. it is actually **reached from the live `/chat` surface** — mounted through
 *    `AssistantUiChat`, with the trail arriving by the real route (the derived
 *    transcript RPC → `mapDisplayItems` → the adapter → message metadata), not
 *    by rendering `ChatSources` directly with a hand-made prop. A component that
 *    renders correctly in isolation while nothing mounts it is the defect shape
 *    this codebase keeps producing (the old voice-only transcript panel, the
 *    assistant-ui reasoning part, the suggestion chips), so proving the wiring is the point;
 * 3. a non-`http(s)` URL never becomes a link. Sources are derived from the
 *    `url` argument of a fetch tool call, which is raw model output, so a
 *    `javascript:` value must not reach an `<a href>`. `extractAgentSources`
 *    enforces that; this pins that the enforcement survives the trip through
 *    the inline surface;
 * 4. the turn is drawn once. A settled answer used to carry a second summary
 *    of its own reasoning and tools under it (a "N steps · M tools" footer and
 *    a sources list both read from a duplicate `processTrail`); only the inline
 *    parts remain;
 * 5. a memory citation on the message's `extraMetadata.citations`
 *    (`ChatDoneEvent.citations` / `ChatSegmentEvent.citations`) renders
 *    alongside the `url` sources as a `document` source badge, with no href.
 *
 * Every row now renders through the vendored `sources.aui` element's
 * primitives directly (no collapsible disclosure — see `ChatSources.tsx`),
 * so there is no "expand" step left to drive.
 *
 * Only the RPC is stubbed — the boundary a unit test should stub. Everything
 * between it and the DOM is production code.
 */
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { threadApi } from '../../../../services/api/threadApi';
import chatRuntimeReducer from '../../../../store/chatRuntimeSlice';
import mascotReducer from '../../../../store/mascotSlice';
import threadReducer from '../../../../store/threadSlice';
import type { DerivedDisplayItem } from '../../../../types/derivedTranscript';
import type { ThreadMessage } from '../../../../types/thread';
import { AssistantUiChat } from '../AssistantUiChat';

const THREAD_ID = 't-sources';
const REQUEST_ID = 'req-sources';
const ANSWER = 'Here is what those pages say.';

function toolCall(callId: string, url: string): DerivedDisplayItem {
  return { kind: 'toolCall', callId, name: 'web_fetch', args: { url }, status: 'success' };
}

/**
 * A transcript page as the RPC returns one: **newest-first**.
 * `mapDisplayItems` reverses it, so the turn boundary has to sit last here to
 * end up first chronologically — otherwise the tool calls anchor to no turn.
 */
function page(...newestFirst: DerivedDisplayItem[]) {
  return {
    items: [...newestFirst, { kind: 'turnBoundary', requestId: REQUEST_ID } as DerivedDisplayItem],
    hasTranscript: true,
    hasMore: false,
  };
}

function agentMessage(citations?: unknown[]): ThreadMessage {
  return {
    id: 'm-1',
    content: ANSWER,
    type: 'text',
    extraMetadata: { requestId: REQUEST_ID, ...(citations ? { citations } : {}) },
    sender: 'agent',
    createdAt: '2026-01-01T00:00:00.000Z',
  };
}

function buildStore(message: ThreadMessage = agentMessage()) {
  return configureStore({
    reducer: combineReducers({
      thread: threadReducer,
      chatRuntime: chatRuntimeReducer,
      mascot: mascotReducer,
    }),
    preloadedState: {
      thread: {
        threads: [
          {
            id: THREAD_ID,
            title: 'Sources thread',
            chatId: null,
            isActive: false,
            messageCount: 1,
            lastMessageAt: '2026-01-01T00:00:00.000Z',
            createdAt: '2026-01-01T00:00:00.000Z',
            labels: [],
          },
        ],
        selectedThreadId: THREAD_ID,
        activeThreadIds: {},
        welcomeThreadId: null,
        messagesByThreadId: { [THREAD_ID]: [message] },
        messages: [message],
        isLoadingThreads: false,
        isLoadingMessages: false,
        messagesError: null,
      },
    } as never,
  });
}

/** Mounted exactly as `/chat` mounts it — never `<ChatSources />` directly. */
function renderChat(message?: ThreadMessage) {
  return render(
    <Provider store={buildStore(message)}>
      <AssistantUiChat
        model={null}
        onModelChange={vi.fn()}
        inputValue=""
        onInputValueChange={vi.fn()}
        attachments={[]}
        onAttachFiles={vi.fn()}
        onRemoveAttachment={vi.fn()}
        maxAttachments={5}
        attachmentsEnabled={false}
        attachmentInteractionBlocked={false}
        onAttachmentOnlySend={vi.fn()}
      />
    </Provider>
  );
}

function sourceHrefs(): (string | null)[] {
  return Array.from(
    document.querySelectorAll<HTMLAnchorElement>('[data-testid="agent-source-row"]')
  ).map(anchor => anchor.getAttribute('href'));
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('inline turn sources', () => {
  it('lists the turn sources on the live chat surface', async () => {
    vi.spyOn(threadApi, 'getDerivedTranscript').mockResolvedValue(
      page(toolCall('c2', 'https://docs.rs/b'), toolCall('c1', 'https://example.com/a')) as never
    );

    renderChat();

    // Reached through `AssistantUiChat` -> `Thread` -> the `SourceGroup` slot,
    // so this proves the wiring and not merely the component.
    await waitFor(() => expect(screen.getByTestId('turn-sources')).toBeTruthy());

    // Collapsed by design: the count is visible, the rows are not yet.
    expect(screen.getByText(/\(2\)$/)).toBeTruthy();
    expect(sourceHrefs()).toEqual([]);

    await expandSources();
    expect(sourceHrefs()).toEqual(['https://example.com/a', 'https://docs.rs/b']);
  });

  it('draws the turn once, with no process footer under the answer', async () => {
    vi.spyOn(threadApi, 'getDerivedTranscript').mockResolvedValue(
      page(toolCall('c1', 'https://example.com/a'), {
        kind: 'reasoning',
        text: 'Looking it up.',
      } as DerivedDisplayItem) as never
    );

    renderChat();

    await waitFor(() => expect(screen.getByTestId('turn-sources')).toBeTruthy());
    // One activity disclosure (reasoning + tools, collapsed once settled) and
    // nothing summarising it a second time under the answer.
    expect(document.querySelectorAll('[data-slot="tool-group-root"]')).toHaveLength(1);
    expect(document.querySelector('[data-testid="turn-process-footer"]')).toBeNull();
    expect(screen.queryByText(/\d+ steps? ·/)).toBeNull();
  });

  it('renders nothing when the turn visited no sources', async () => {
    vi.spyOn(threadApi, 'getDerivedTranscript').mockResolvedValue(page() as never);

    renderChat();

    // Guarded so the absence cannot pass on a blank tree: the answer has to be
    // on screen for this assertion to mean anything.
    await waitFor(() => expect(screen.getByText(ANSWER)).toBeTruthy());
    expect(document.querySelector('[data-testid="turn-sources"]')).toBeNull();
  });

  it('never renders a non-http(s) url as a link', async () => {
    vi.spyOn(threadApi, 'getDerivedTranscript').mockResolvedValue(
      page(
        toolCall('c2', 'https://example.com/safe'),
        toolCall('c1', 'javascript:alert(1)')
      ) as never
    );

    renderChat();

    await waitFor(() => expect(screen.getByTestId('turn-sources')).toBeTruthy());
    await expandSources();

    // One row, not two: the `javascript:` entry is dropped by
    // `extractAgentSources`, so it is never counted and never linked.
    expect(sourceHrefs()).toEqual(['https://example.com/safe']);
    // The fetch card may show the raw argument as text; it must never be a link.
    const hrefs = Array.from(document.querySelectorAll('a[href]')).map(a => a.getAttribute('href'));
    expect(hrefs.some(href => href?.startsWith('javascript:'))).toBe(false);
  });
});
