import { configureStore } from '@reduxjs/toolkit';
import { describe, expect, it } from 'vitest';

import type { PersistedTurnState } from '../types/turnState';
import chatRuntimeReducer, {
  hydrateRuntimeFromSnapshot,
  streamDeltaReceived,
} from './chatRuntimeSlice';

/**
 * Regression coverage for the reasoning timing the "Thinking… Ns" badge and
 * the settled "Thought for Ns" label read. Before this, no timing existed
 * for reasoning at all, so the panel could never show a duration.
 */

function makeStore() {
  return configureStore({ reducer: { chatRuntime: chatRuntimeReducer } });
}

const think = (delta: string, at: number | undefined, round = 1, requestId = 'req-1') =>
  streamDeltaReceived({ threadId: 't', requestId, round, delta, channel: 'thinking', at });

describe('streamDeltaReceived — reasoning timing', () => {
  it('stamps a thinking block with its first delta and advances its end', () => {
    const store = makeStore();
    store.dispatch(think('Let me ', 1_000));
    store.dispatch(think('check.', 4_500));

    const [item] = store.getState().chatRuntime.processingByThread.t;
    expect(item).toMatchObject({
      kind: 'thinking',
      text: 'Let me check.',
      startedAt: 1_000,
      endedAt: 4_500,
    });
    const streaming = store.getState().chatRuntime.streamingAssistantByThread.t;
    expect(streaming).toMatchObject({ thinkingStartedAt: 1_000, thinkingEndedAt: 4_500 });
  });

  it('opens a new timed block per round', () => {
    const store = makeStore();
    store.dispatch(think('one', 1_000, 1));
    store.dispatch(think('two', 9_000, 2));
    const items = store.getState().chatRuntime.processingByThread.t;
    expect(items).toHaveLength(2);
    expect(items[1]).toMatchObject({ startedAt: 9_000, endedAt: 9_000 });
    // The turn-level span covers both rounds.
    expect(store.getState().chatRuntime.streamingAssistantByThread.t).toMatchObject({
      thinkingStartedAt: 1_000,
      thinkingEndedAt: 9_000,
    });
  });

  it('keeps thinking timing across content deltas in the same turn', () => {
    const store = makeStore();
    store.dispatch(think('hmm', 2_000));
    store.dispatch(
      streamDeltaReceived({
        threadId: 't',
        requestId: 'req-1',
        round: 1,
        delta: 'Answer',
        channel: 'content',
        at: 7_000,
      })
    );
    expect(store.getState().chatRuntime.streamingAssistantByThread.t).toMatchObject({
      content: 'Answer',
      thinkingStartedAt: 2_000,
      thinkingEndedAt: 2_000,
    });
  });

  it('resets timing when a new request starts', () => {
    const store = makeStore();
    store.dispatch(think('old', 1_000, 1, 'req-1'));
    store.dispatch(think('new', 50_000, 1, 'req-2'));
    expect(store.getState().chatRuntime.streamingAssistantByThread.t).toMatchObject({
      requestId: 'req-2',
      thinkingStartedAt: 50_000,
    });
  });

  it('records no timing for deltas dispatched without a timestamp', () => {
    const store = makeStore();
    store.dispatch(think('untimed', undefined));
    const [item] = store.getState().chatRuntime.processingByThread.t;
    expect(item).not.toHaveProperty('startedAt');
    expect(store.getState().chatRuntime.streamingAssistantByThread.t).not.toHaveProperty(
      'thinkingStartedAt'
    );
  });
});

describe('hydrateRuntimeFromSnapshot — reasoning timing', () => {
  it('keeps persisted startedAt / endedAt so a reload still says "Thought for Ns"', () => {
    const store = makeStore();
    const snapshot: PersistedTurnState = {
      threadId: 't-h',
      requestId: 'req-1',
      lifecycle: 'completed',
      iteration: 1,
      maxIterations: 10,
      streamingText: '',
      thinking: '',
      toolTimeline: [],
      transcript: [
        { kind: 'thinking', round: 1, seq: 0, text: 'plan', startedAt: 1_000, endedAt: 13_000 },
      ],
      startedAt: '2026-09-24T00:00:00Z',
      updatedAt: '2026-09-24T00:00:00Z',
    };
    store.dispatch(hydrateRuntimeFromSnapshot({ snapshot }));
    expect(store.getState().chatRuntime.processingByThread['t-h'][0]).toMatchObject({
      startedAt: 1_000,
      endedAt: 13_000,
    });
  });
});
