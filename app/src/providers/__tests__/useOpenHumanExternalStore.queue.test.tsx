/**
 * The external store opts into assistant-ui's message queue over the core's
 * run queue: `queue.items` come from `queueSlice`, and because the runtime
 * sends through the queue once one exists, both lanes must still reach the
 * surface's own send path (which picks follow-up vs normal `queue_mode`).
 */
import type { AppendMessage } from '@assistant-ui/react';
import { configureStore } from '@reduxjs/toolkit';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import chatRuntimeReducer from '../../store/chatRuntimeSlice';
import queueReducer, { queueItemQueued } from '../../store/queueSlice';
import threadReducer from '../../store/threadSlice';
import { registerChatSurface } from '../chatSurfaceHandlers';
import { useOpenHumanExternalStore } from '../useOpenHumanExternalStore';

vi.mock('../../services/api/threadApi', () => ({
  threadApi: {
    getDerivedTranscript: vi
      .fn()
      .mockResolvedValue({
        threadId: 't-queue',
        items: [],
        total: 0,
        hasMore: false,
        hasTranscript: false,
      }),
  },
}));

const THREAD_ID = 't-queue';

const appended = (text: string) =>
  ({
    role: 'user',
    content: [{ type: 'text', text }],
    parentId: null,
    sourceId: null,
    attachments: [],
    metadata: { custom: {} },
    createdAt: new Date(),
  }) as unknown as AppendMessage;

function mount() {
  const store = configureStore({
    reducer: { thread: threadReducer, chatRuntime: chatRuntimeReducer, queue: queueReducer },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store}>{children}</Provider>
  );
  return { store, ...renderHook(() => useOpenHumanExternalStore(THREAD_ID), { wrapper }) };
}

describe('useOpenHumanExternalStore — queue', () => {
  let sent: string[];

  beforeEach(() => {
    sent = [];
    registerChatSurface(THREAD_ID, {
      send: async (text: string) => {
        sent.push(text);
      },
    });
  });

  it("exposes this thread's core queue items", () => {
    const { store, result } = mount();
    expect(result.current.queue?.items).toEqual([]);

    act(() => {
      store.dispatch(
        queueItemQueued({ threadId: THREAD_ID, item: { id: 'q1', text_preview: 'next' } })
      );
      store.dispatch(
        queueItemQueued({ threadId: 'other', item: { id: 'q2', text_preview: 'elsewhere' } })
      );
    });

    expect(result.current.queue?.items.map(item => item.id)).toEqual(['q1']);
  });

  it('routes both queue lanes through the surface send', async () => {
    const { result } = mount();

    act(() => {
      result.current.queue?.enqueue(appended('while idle'));
      result.current.queue?.steer(appended('while running'));
    });

    await waitFor(() => expect(sent).toEqual(['while idle', 'while running']));
  });
});
