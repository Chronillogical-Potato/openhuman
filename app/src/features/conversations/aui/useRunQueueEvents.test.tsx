import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { type QueueEventListeners, subscribeQueueEvents } from '../../../services/chatService';
import chatRuntimeReducer from '../../../store/chatRuntimeSlice';
import queueReducer, { pendingFollowupAdded } from '../../../store/queueSlice';
import { useRunQueueEvents } from './useRunQueueEvents';

// The socket is the one external boundary here; capture what the hook registers.
vi.mock('../../../services/chatService', () => ({ subscribeQueueEvents: vi.fn() }));

function setup() {
  const store = configureStore({
    reducer: combineReducers({ chatRuntime: chatRuntimeReducer, queue: queueReducer }),
  });
  let listeners: QueueEventListeners = {};
  const unsubscribe = vi.fn();
  vi.mocked(subscribeQueueEvents).mockImplementation(l => {
    listeners = l;
    return unsubscribe;
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store}>{children}</Provider>
  );
  const hook = renderHook(({ enabled }) => useRunQueueEvents(enabled), {
    wrapper,
    initialProps: { enabled: true },
  });
  return { store, listeners: () => listeners, unsubscribe, hook };
}

const event = (id: string, text: string) => ({
  thread_id: 't1',
  queue_item: { id, text_preview: text },
});

describe('useRunQueueEvents', () => {
  beforeEach(() => vi.mocked(subscribeQueueEvents).mockReset());

  it('mirrors queued and delivered events into the queue slice', () => {
    const { store, listeners } = setup();

    act(() => listeners().onQueued?.(event('q1', 'one')));
    act(() => listeners().onQueued?.(event('q2', 'two')));
    expect(store.getState().queue.itemsByThread.t1.map(i => i.id)).toEqual(['q1', 'q2']);

    act(() => listeners().onDelivered?.(event('q1', 'one')));
    expect(store.getState().queue.itemsByThread.t1.map(i => i.id)).toEqual(['q2']);
  });

  it('a core-side removal also drops the matching pending follow-up', () => {
    const { store, listeners } = setup();
    store.dispatch(
      pendingFollowupAdded({
        threadId: 't1',
        text: 'gone',
        message: {
          id: 'm1',
          content: 'gone',
          type: 'text',
          extraMetadata: {},
          sender: 'user',
          createdAt: '2026-01-01T00:00:00.000Z',
        },
      })
    );
    act(() => listeners().onQueued?.(event('q1', 'gone')));
    act(() => listeners().onRemoved?.(event('q1', 'gone')));

    expect(store.getState().queue.itemsByThread.t1).toBeUndefined();
    expect(store.getState().queue.pendingFollowupsByThread.t1).toBeUndefined();
  });

  it('does not subscribe while disabled and unsubscribes when disabled', () => {
    const { hook, unsubscribe } = setup();
    expect(subscribeQueueEvents).toHaveBeenCalledTimes(1);

    hook.rerender({ enabled: false });
    expect(unsubscribe).toHaveBeenCalledTimes(1);
    expect(subscribeQueueEvents).toHaveBeenCalledTimes(1);
  });
});
