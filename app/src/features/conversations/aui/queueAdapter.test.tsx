import type { AppendMessage } from '@assistant-ui/react';
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { chatRemoveQueueItem } from '../../../services/chatService';
import chatRuntimeReducer from '../../../store/chatRuntimeSlice';
import queueReducer, { pendingFollowupAdded, queueItemQueued } from '../../../store/queueSlice';
import { buildOpenHumanQueueAdapter, useOpenHumanQueueAdapter } from './queueAdapter';

vi.mock('../../../services/chatService', () => ({ chatRemoveQueueItem: vi.fn() }));

const append = (text: string): AppendMessage =>
  ({ role: 'user', content: [{ type: 'text', text }] }) as unknown as AppendMessage;

function buildStore() {
  return configureStore({
    reducer: combineReducers({ chatRuntime: chatRuntimeReducer, queue: queueReducer }),
  });
}

describe('buildOpenHumanQueueAdapter', () => {
  it('projects core items onto assistant-ui queue items', () => {
    const adapter = buildOpenHumanQueueAdapter({
      items: [{ id: 'q1', lane: null, textPreview: 'and the pricing?' }],
      send: vi.fn(),
      remove: vi.fn(),
    });

    expect(adapter.items).toEqual([
      {
        id: 'q1',
        prompt: 'and the pricing?',
        parts: [{ type: 'text', text: 'and the pricing?' }],
      },
    ]);
    expect(adapter.steerItems).toEqual([]);
  });

  it('sends both lanes through the host send path, which owns queue_mode', () => {
    const send = vi.fn().mockResolvedValue(undefined);
    const adapter = buildOpenHumanQueueAdapter({ items: [], send, remove: vi.fn() });

    adapter.enqueue(append('idle send'));
    adapter.steer(append('send while running'));

    expect(send.mock.calls.map(([m]) => (m as AppendMessage).content)).toEqual([
      [{ type: 'text', text: 'idle send' }],
      [{ type: 'text', text: 'send while running' }],
    ]);
  });

  it('swallows a rejected send (the host surfaces it) instead of an unhandled rejection', async () => {
    const send = vi.fn().mockRejectedValue(new Error('no surface'));
    const adapter = buildOpenHumanQueueAdapter({ items: [], send, remove: vi.fn() });

    expect(() => adapter.enqueue(append('x'))).not.toThrow();
    await Promise.resolve();
    expect(send).toHaveBeenCalledTimes(1);
  });

  it('forwards removal and ignores move/edit, which the core queue cannot do', () => {
    const remove = vi.fn();
    const send = vi.fn();
    const adapter = buildOpenHumanQueueAdapter({ items: [], send, remove });

    adapter.remove('q1');
    adapter.move('q1', { lane: 'steer' });
    adapter.edit('q1', append('edited'));

    expect(remove).toHaveBeenCalledWith('q1');
    expect(send).not.toHaveBeenCalled();
  });
});

describe('useOpenHumanQueueAdapter', () => {
  beforeEach(() => vi.mocked(chatRemoveQueueItem).mockReset());

  const wrapperFor =
    (store: ReturnType<typeof buildStore>) =>
    ({ children }: { children: ReactNode }) => <Provider store={store}>{children}</Provider>;

  it('reads the thread queue from the store and keeps items referentially stable', () => {
    const store = buildStore();
    store.dispatch(queueItemQueued({ threadId: 't1', item: { id: 'q1', text_preview: 'hi' } }));
    const { result, rerender } = renderHook(() => useOpenHumanQueueAdapter('t1', vi.fn()), {
      wrapper: wrapperFor(store),
    });

    const first = result.current.items;
    rerender();
    expect(result.current.items).toBe(first);
    expect(first.map(i => i.id)).toEqual(['q1']);
  });

  it('is empty without a thread', () => {
    const { result } = renderHook(() => useOpenHumanQueueAdapter(null, vi.fn()), {
      wrapper: wrapperFor(buildStore()),
    });
    expect(result.current.items).toEqual([]);
  });

  it('removes an item (and its pending follow-up) once the core confirms', async () => {
    vi.mocked(chatRemoveQueueItem).mockResolvedValue(true);
    const store = buildStore();
    store.dispatch(
      pendingFollowupAdded({
        threadId: 't1',
        text: 'drop me',
        message: {
          id: 'm1',
          content: 'drop me',
          type: 'text',
          extraMetadata: {},
          sender: 'user',
          createdAt: '2026-01-01T00:00:00.000Z',
        },
      })
    );
    store.dispatch(queueItemQueued({ threadId: 't1', item: { id: 'q1', text_preview: 'drop me' } }));
    const { result } = renderHook(() => useOpenHumanQueueAdapter('t1', vi.fn()), {
      wrapper: wrapperFor(store),
    });

    act(() => result.current.remove('q1'));

    await waitFor(() => expect(store.getState().queue.itemsByThread.t1).toBeUndefined());
    expect(chatRemoveQueueItem).toHaveBeenCalledWith('t1', 'q1');
    expect(store.getState().queue.pendingFollowupsByThread.t1).toBeUndefined();
  });

  it('keeps the item when the core does not confirm the removal', async () => {
    vi.mocked(chatRemoveQueueItem).mockResolvedValue(false);
    const store = buildStore();
    store.dispatch(queueItemQueued({ threadId: 't1', item: { id: 'q1', text_preview: 'stay' } }));
    const { result } = renderHook(() => useOpenHumanQueueAdapter('t1', vi.fn()), {
      wrapper: wrapperFor(store),
    });

    await act(async () => {
      result.current.remove('q1');
      await Promise.resolve();
    });

    expect(chatRemoveQueueItem).toHaveBeenCalledWith('t1', 'q1');
    expect(store.getState().queue.itemsByThread.t1).toHaveLength(1);
  });
});
