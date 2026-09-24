import { describe, expect, it } from 'vitest';

import type { ThreadMessage } from '../types/thread';
import {
  beginInferenceTurn,
  clearAllChatRuntime,
  clearRuntimeForThread,
  endInferenceTurn,
} from './chatRuntimeSlice';
import reducer, {
  clipQueuePreview,
  pendingFollowupAdded,
  queueItemDelivered,
  queueItemQueued,
  queueItemRemoved,
} from './queueSlice';

const message = (id: string, content: string): ThreadMessage => ({
  id,
  content,
  type: 'text',
  extraMetadata: {},
  sender: 'user',
  createdAt: '2026-01-01T00:00:00.000Z',
});

const queued = (threadId: string, id: string, text: string) =>
  queueItemQueued({ threadId, item: { id, text_preview: text } });

describe('queueSlice — core run-queue items', () => {
  it('appends queued items in order per thread', () => {
    let state = reducer(undefined, queued('t1', 'q1', 'first'));
    state = reducer(state, queued('t1', 'q2', 'second'));
    state = reducer(state, queued('t2', 'q3', 'other'));

    expect(state.itemsByThread.t1).toEqual([
      { id: 'q1', lane: null, textPreview: 'first' },
      { id: 'q2', lane: null, textPreview: 'second' },
    ]);
    expect(state.itemsByThread.t2).toHaveLength(1);
  });

  it('ignores a duplicate queued event for the same item id', () => {
    let state = reducer(undefined, queued('t1', 'q1', 'first'));
    state = reducer(state, queued('t1', 'q1', 'first'));

    expect(state.itemsByThread.t1).toHaveLength(1);
  });

  it('keeps the lane and falls back to an empty preview when the core omits one', () => {
    const state = reducer(
      undefined,
      queueItemQueued({ threadId: 't1', item: { id: 'q1', lane: 'steer' } })
    );

    expect(state.itemsByThread.t1).toEqual([{ id: 'q1', lane: 'steer', textPreview: '' }]);
  });

  it('drops a delivered item and prunes the empty bucket', () => {
    let state = reducer(undefined, queued('t1', 'q1', 'first'));
    state = reducer(state, queued('t1', 'q2', 'second'));

    state = reducer(state, queueItemDelivered({ threadId: 't1', itemId: 'q1' }));
    expect(state.itemsByThread.t1.map(i => i.id)).toEqual(['q2']);

    state = reducer(state, queueItemDelivered({ threadId: 't1', itemId: 'q2' }));
    expect(state.itemsByThread.t1).toBeUndefined();
  });

  it('keeps pending follow-ups when an item is delivered (they persist on turn end)', () => {
    let state = reducer(undefined, pendingFollowupAdded({ threadId: 't1', message: message('m1', 'hi'), text: 'hi' }));
    state = reducer(state, queued('t1', 'q1', 'hi'));
    state = reducer(state, queueItemDelivered({ threadId: 't1', itemId: 'q1' }));

    expect(state.pendingFollowupsByThread.t1.map(p => p.message.id)).toEqual(['m1']);
  });
});

describe('queueSlice — pending follow-up persistence', () => {
  it('records follow-ups in send order with a core-shaped preview', () => {
    let state = reducer(
      undefined,
      pendingFollowupAdded({ threadId: 't1', message: message('m1', 'one'), text: 'one' })
    );
    state = reducer(
      state,
      pendingFollowupAdded({ threadId: 't1', message: message('m2', 'two'), text: 'two' })
    );

    expect(state.pendingFollowupsByThread.t1.map(p => [p.message.id, p.preview])).toEqual([
      ['m1', 'one'],
      ['m2', 'two'],
    ]);
  });

  it('removing an item also drops the follow-up whose preview it carries', () => {
    let state = reducer(
      undefined,
      pendingFollowupAdded({ threadId: 't1', message: message('m1', 'keep'), text: 'keep' })
    );
    state = reducer(
      state,
      pendingFollowupAdded({ threadId: 't1', message: message('m2', 'drop'), text: 'drop' })
    );
    state = reducer(state, queued('t1', 'q1', 'keep'));
    state = reducer(state, queued('t1', 'q2', 'drop'));

    state = reducer(state, queueItemRemoved({ threadId: 't1', itemId: 'q2' }));

    expect(state.itemsByThread.t1.map(i => i.id)).toEqual(['q1']);
    expect(state.pendingFollowupsByThread.t1.map(p => p.message.id)).toEqual(['m1']);
  });

  it('a removal for an unknown item leaves pending follow-ups alone', () => {
    let state = reducer(
      undefined,
      pendingFollowupAdded({ threadId: 't1', message: message('m1', 'x'), text: 'x' })
    );
    state = reducer(state, queueItemRemoved({ threadId: 't1', itemId: 'nope' }));

    expect(state.pendingFollowupsByThread.t1).toHaveLength(1);
  });

  it('prunes the pending bucket once its last follow-up is removed', () => {
    let state = reducer(
      undefined,
      pendingFollowupAdded({ threadId: 't1', message: message('m1', 'x'), text: 'x' })
    );
    state = reducer(state, queued('t1', 'q1', 'x'));
    state = reducer(state, queueItemRemoved({ threadId: 't1', itemId: 'q1' }));

    expect(state.pendingFollowupsByThread.t1).toBeUndefined();
    expect(state.itemsByThread.t1).toBeUndefined();
  });
});

describe('queueSlice — chat runtime lifecycle', () => {
  const seeded = () => {
    let state = reducer(undefined, queued('t1', 'q1', 'a'));
    state = reducer(state, queued('t2', 'q2', 'b'));
    state = reducer(
      state,
      pendingFollowupAdded({ threadId: 't1', message: message('m1', 'a'), text: 'a' })
    );
    return reducer(
      state,
      pendingFollowupAdded({ threadId: 't2', message: message('m2', 'b'), text: 'b' })
    );
  };

  it('endInferenceTurn clears the thread queue (its follow-ups are being dispatched)', () => {
    let state = reducer(seeded(), beginInferenceTurn({ threadId: 't1' }));
    state = reducer(state, endInferenceTurn({ threadId: 't1' }));

    expect(state.itemsByThread.t1).toBeUndefined();
    expect(state.pendingFollowupsByThread.t1).toBeUndefined();
    expect(state.itemsByThread.t2).toBeDefined();
  });

  it('clearRuntimeForThread clears one thread, clearAllChatRuntime clears all', () => {
    const perThread = reducer(seeded(), clearRuntimeForThread({ threadId: 't1' }));
    expect(perThread.itemsByThread.t1).toBeUndefined();
    expect(perThread.pendingFollowupsByThread.t1).toBeUndefined();
    expect(perThread.pendingFollowupsByThread.t2).toBeDefined();

    const all = reducer(seeded(), clearAllChatRuntime());
    expect(all).toEqual({ itemsByThread: {}, pendingFollowupsByThread: {} });
  });
});

describe('clipQueuePreview', () => {
  it('matches the core clip: 80 code points, then an ellipsis', () => {
    expect(clipQueuePreview('short')).toBe('short');
    expect(clipQueuePreview('x'.repeat(80))).toBe('x'.repeat(80));
    expect(clipQueuePreview('x'.repeat(81))).toBe(`${'x'.repeat(80)}…`);
    // Astral characters count once, as Rust's `chars()` does.
    expect(clipQueuePreview('😀'.repeat(81))).toBe(`${'😀'.repeat(80)}…`);
  });
});
