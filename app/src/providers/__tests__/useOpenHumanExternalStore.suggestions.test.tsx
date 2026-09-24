/**
 * The adapter's `suggestions` field — the one inlet for BOTH chip surfaces.
 *
 * Welcome chips (`thread.tsx`, empty thread) and follow-up chips
 * (`follow-up-suggestions.tsx`, after a settled turn) read the same
 * `s.thread.suggestions`, so the adapter must never hand out a list that one
 * surface would show where the other belongs. These tests pin that partition:
 * welcome chips only on an empty thread, the core's follow-ups only after a
 * settled turn that ended on an assistant reply, and never both.
 */
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';

import chatRuntimeReducer, { beginInferenceTurn } from '../../store/chatRuntimeSlice';
import followupSuggestionsReducer, {
  followupSuggestionsReceived,
} from '../../store/followupSuggestionsSlice';
import threadReducer from '../../store/threadSlice';
import type { ThreadMessage } from '../../types/thread';
import { useOpenHumanExternalStore } from '../useOpenHumanExternalStore';

vi.mock('../../services/api/threadApi', () => ({
  threadApi: {
    getDerivedTranscript: vi
      .fn()
      .mockResolvedValue({ items: [], total: 0, hasMore: false, hasTranscript: false }),
  },
}));

const THREAD_ID = 't-suggest';
const WELCOME_FIRST = "What's on my calendar today?";

const userMessage: ThreadMessage = {
  id: 'u-1',
  sender: 'user',
  type: 'text',
  content: 'What is on today?',
  extraMetadata: {},
  createdAt: '2026-01-01T00:00:00.000Z',
};
const agentMessage: ThreadMessage = {
  id: 'a-1',
  sender: 'agent',
  type: 'text',
  content: 'Two meetings.',
  extraMetadata: {},
  createdAt: '2026-01-01T00:01:00.000Z',
};

const STORED = [
  { prompt: 'Move the second meeting to Friday', label: 'Reschedule' },
  { prompt: 'Who is attending?' },
];

function buildStore({
  messages,
  running = false,
  stored = false,
}: {
  messages: ThreadMessage[];
  running?: boolean;
  stored?: boolean;
}) {
  const store = configureStore({
    reducer: combineReducers({
      thread: threadReducer,
      chatRuntime: chatRuntimeReducer,
      followupSuggestions: followupSuggestionsReducer,
    }),
    preloadedState: {
      thread: {
        ...threadReducer(undefined, { type: '@@INIT' }),
        selectedThreadId: THREAD_ID,
        messagesByThreadId: { [THREAD_ID]: messages },
        messages,
      },
    } as never,
  });
  if (running) store.dispatch(beginInferenceTurn({ threadId: THREAD_ID }));
  if (stored) {
    store.dispatch(
      followupSuggestionsReceived({ threadId: THREAD_ID, requestId: 'r1', suggestions: STORED })
    );
  }
  return store;
}

function mountAdapter(store: ReturnType<typeof buildStore>, welcomeSuggestions?: boolean) {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store}>{children}</Provider>
  );
  return renderHook(
    () =>
      useOpenHumanExternalStore(
        THREAD_ID,
        welcomeSuggestions === undefined ? undefined : { welcomeSuggestions }
      ),
    { wrapper }
  );
}

const prompts = (list: readonly { prompt: string }[]) => list.map(s => s.prompt);

describe('useOpenHumanExternalStore — suggestions', () => {
  it('offers the welcome chips on an empty thread', () => {
    const { result } = mountAdapter(buildStore({ messages: [] }));

    expect(result.current.suggestions).toHaveLength(6);
    expect(result.current.suggestions[0]).toEqual({ prompt: WELCOME_FIRST });
  });

  it('never offers follow-ups on an empty thread, even if some are stored', () => {
    const { result } = mountAdapter(buildStore({ messages: [], stored: true }));

    expect(prompts(result.current.suggestions)).not.toContain(STORED[0].prompt);
    expect(result.current.suggestions[0]).toEqual({ prompt: WELCOME_FIRST });
  });

  it('offers the stored follow-ups after a settled turn, labelled chips titled by their label', () => {
    const { result } = mountAdapter(
      buildStore({ messages: [userMessage, agentMessage], stored: true })
    );

    expect(result.current.suggestions).toEqual([
      { prompt: 'Move the second meeting to Friday', title: 'Reschedule' },
      { prompt: 'Who is attending?' },
    ]);
  });

  it('offers nothing after a settled turn with no stored follow-ups (no welcome chips under a turn)', () => {
    const { result } = mountAdapter(buildStore({ messages: [userMessage, agentMessage] }));

    expect(result.current.suggestions).toEqual([]);
  });

  it('offers nothing while a turn is running, even with follow-ups stored', () => {
    const store = buildStore({ messages: [userMessage, agentMessage], stored: true });
    const { result } = mountAdapter(store);
    expect(result.current.suggestions).toHaveLength(2);

    // `stored` precedes the send here on purpose: the reducer clears on send,
    // so re-store to prove the RUNNING gate alone keeps them hidden.
    act(() => {
      store.dispatch(beginInferenceTurn({ threadId: THREAD_ID }));
      store.dispatch(
        followupSuggestionsReceived({ threadId: THREAD_ID, requestId: 'r1', suggestions: STORED })
      );
    });

    expect(result.current.isRunning).toBe(true);
    expect(result.current.suggestions).toEqual([]);
  });

  it('offers nothing when the settled thread ends on a user message', () => {
    const { result } = mountAdapter(
      buildStore({ messages: [agentMessage, userMessage], stored: true })
    );

    expect(result.current.suggestions).toEqual([]);
  });

  it('drops the follow-ups as soon as the next send starts', () => {
    const store = buildStore({ messages: [userMessage, agentMessage], stored: true });
    const { result } = mountAdapter(store);
    expect(result.current.suggestions).toHaveLength(2);

    act(() => {
      store.dispatch(beginInferenceTurn({ threadId: THREAD_ID }));
    });

    expect(result.current.suggestions).toEqual([]);
    expect(store.getState().followupSuggestions.byThread[THREAD_ID]).toBeUndefined();
  });

  it('keeps follow-ups on a surface that opts out of welcome chips', () => {
    const empty = mountAdapter(buildStore({ messages: [] }), false);
    expect(empty.result.current.suggestions).toEqual([]);

    const settled = mountAdapter(
      buildStore({ messages: [userMessage, agentMessage], stored: true }),
      false
    );
    expect(settled.result.current.suggestions).toHaveLength(2);
  });

  it('tolerates a store without the follow-up slice (no chips, no crash)', () => {
    const store = configureStore({
      reducer: combineReducers({ thread: threadReducer, chatRuntime: chatRuntimeReducer }),
      preloadedState: {
        thread: {
          ...threadReducer(undefined, { type: '@@INIT' }),
          messagesByThreadId: { [THREAD_ID]: [userMessage, agentMessage] },
        },
      } as never,
    });
    const wrapper = ({ children }: { children: ReactNode }) => (
      <Provider store={store}>{children}</Provider>
    );
    const { result } = renderHook(() => useOpenHumanExternalStore(THREAD_ID), { wrapper });

    expect(result.current.suggestions).toEqual([]);
  });
});
