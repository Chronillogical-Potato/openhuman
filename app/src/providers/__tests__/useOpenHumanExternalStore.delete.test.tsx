/**
 * `onDelete` — the adapter callback backing the vendored `StoppedRun`
 * element's Discard action (`components/assistant-ui/thread.tsx`'s
 * `StoppedRunSlot`).
 *
 * There is no backend RPC to delete a persisted turn, so this only trims the
 * LOCAL cache via `truncateMessagesFrom` (the same helper `onEdit`/`onReload`
 * use) — the core keeps the row.
 *
 * Supplying `onDelete` at all is load-bearing: `ExternalStoreThreadRuntimeCore
 * .deleteMessage` checks for it FIRST, ahead of a `setMessages`-based
 * fallback that already reported `capabilities.delete: true` (because
 * `setMessages` is supplied as a no-op stub for the branch picker) but would
 * silently undo itself the next render — see `onDelete`'s own docstring in
 * `useOpenHumanExternalStore.ts`.
 */
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';

import chatRuntimeReducer from '../../store/chatRuntimeSlice';
import threadReducer from '../../store/threadSlice';
import type { ThreadMessage } from '../../types/thread';
import { useOpenHumanExternalStore } from '../useOpenHumanExternalStore';

vi.mock('../../services/api/threadApi', () => ({
  threadApi: {
    getDerivedTranscript: vi
      .fn()
      .mockResolvedValue({
        threadId: 't-delete',
        items: [],
        total: 0,
        hasMore: false,
        hasTranscript: false,
      }),
  },
}));

const THREAD_ID = 't-delete';

const messages: ThreadMessage[] = [
  {
    id: 'u-1',
    sender: 'user',
    type: 'text',
    content: 'go',
    extraMetadata: {},
    createdAt: '2026-01-01T00:00:00.000Z',
  },
  {
    id: 'a-1',
    sender: 'agent',
    type: 'text',
    content: 'partial reply that got cut off',
    extraMetadata: { stopped: true, cancelReason: 'user_stop' },
    createdAt: '2026-01-01T00:01:00.000Z',
  },
];

function buildStore() {
  return configureStore({
    reducer: combineReducers({ thread: threadReducer, chatRuntime: chatRuntimeReducer }),
    preloadedState: {
      thread: {
        ...threadReducer(undefined, { type: '@@INIT' }),
        selectedThreadId: THREAD_ID,
        messagesByThreadId: { [THREAD_ID]: messages },
        messages,
      },
    } as never,
  });
}

function mountAdapter(store: ReturnType<typeof buildStore>) {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store}>{children}</Provider>
  );
  return renderHook(() => useOpenHumanExternalStore(THREAD_ID), { wrapper });
}

describe('onDelete — discarding a stopped partial reply', () => {
  it('drops the message from the local cache', () => {
    const store = buildStore();
    const { result } = mountAdapter(store);

    result.current.onDelete('a-1');

    expect(store.getState().thread.messagesByThreadId[THREAD_ID]).toEqual([messages[0]]);
  });

  it('is a no-op with no thread selected', () => {
    const store = buildStore();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <Provider store={store}>{children}</Provider>
    );
    const { result } = renderHook(() => useOpenHumanExternalStore(null), { wrapper });

    expect(() => result.current.onDelete('a-1')).not.toThrow();
    expect(store.getState().thread.messagesByThreadId[THREAD_ID]).toEqual(messages);
  });
});
