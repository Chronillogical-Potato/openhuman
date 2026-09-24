import { describe, expect, it, vi } from 'vitest';

import {
  type CoalescibleDelta,
  createChatDeltaCoalescer,
  type DeltaChannel,
  type FlushScheduler,
  withCoalescedDeltas,
} from './chatDeltaCoalescer';

/** A scheduler the test fires by hand: one pending flush at a time. */
function manualScheduler() {
  let pending: (() => void) | null = null;
  const schedule: FlushScheduler = flush => {
    pending = flush;
    return () => {
      pending = null;
    };
  };
  return {
    schedule,
    frame: () => {
      const run = pending;
      pending = null;
      run?.();
    },
    get scheduled() {
      return pending !== null;
    },
  };
}

const delta = (over: Partial<CoalescibleDelta>): CoalescibleDelta => ({
  thread_id: 't',
  request_id: 'r',
  round: 1,
  delta: '',
  ...over,
});

describe('createChatDeltaCoalescer', () => {
  it('delivers a frame of same-key deltas as ONE merged delta', () => {
    const frames = manualScheduler();
    const deliver = vi.fn();
    const coalescer = createChatDeltaCoalescer(deliver, frames.schedule);

    for (const piece of ['He', 'll', 'o']) coalescer.push('content', delta({ delta: piece }));
    expect(deliver).not.toHaveBeenCalled();

    frames.frame();
    expect(deliver).toHaveBeenCalledTimes(1);
    expect(deliver).toHaveBeenCalledWith('content', delta({ delta: 'Hello' }));
  });

  it('keeps interleaved channels, rounds and threads in arrival order', () => {
    const frames = manualScheduler();
    const delivered: [DeltaChannel, string][] = [];
    const coalescer = createChatDeltaCoalescer<CoalescibleDelta>(
      (channel, event) => delivered.push([channel, `${event.thread_id}/${event.round}:${event.delta}`]),
      frames.schedule
    );

    coalescer.push('thinking', delta({ delta: 'a' }));
    coalescer.push('thinking', delta({ delta: 'b' }));
    coalescer.push('content', delta({ delta: 'c' }));
    coalescer.push('content', delta({ round: 2, delta: 'd' }));
    coalescer.push('content', delta({ thread_id: 'u', round: 2, delta: 'e' }));
    coalescer.push('content', delta({ round: 2, delta: 'f' }));
    frames.frame();

    expect(delivered).toEqual([
      ['thinking', 't/1:ab'],
      ['content', 't/1:c'],
      ['content', 't/2:d'],
      ['content', 'u/2:e'],
      ['content', 't/2:f'],
    ]);
  });

  it('flushes synchronously on demand and cancels the scheduled frame', () => {
    const frames = manualScheduler();
    const deliver = vi.fn();
    const coalescer = createChatDeltaCoalescer(deliver, frames.schedule);

    coalescer.push('content', delta({ delta: 'x' }));
    expect(frames.scheduled).toBe(true);
    coalescer.flush();
    expect(deliver).toHaveBeenCalledTimes(1);
    expect(frames.scheduled).toBe(false);
  });
});

describe('withCoalescedDeltas', () => {
  it('flushes pending text before any other event, so a tool call lands after its text', () => {
    const frames = manualScheduler();
    const order: string[] = [];
    const { listeners } = withCoalescedDeltas(
      {
        onTextDelta: (event: CoalescibleDelta) => order.push(`text:${event.delta}`),
        onToolCall: () => order.push('tool_call'),
      },
      frames.schedule
    );

    listeners.onTextDelta?.(delta({ delta: 'Let me ' }));
    listeners.onTextDelta?.(delta({ delta: 'check.' }));
    (listeners.onToolCall as () => void)();

    expect(order).toEqual(['text:Let me check.', 'tool_call']);
  });

  it('dispose delivers whatever is still queued', () => {
    const frames = manualScheduler();
    const onThinkingDelta = vi.fn();
    const { listeners, dispose } = withCoalescedDeltas({ onThinkingDelta }, frames.schedule);

    listeners.onThinkingDelta?.(delta({ delta: 'hmm' }));
    dispose();
    expect(onThinkingDelta).toHaveBeenCalledWith(delta({ delta: 'hmm' }));
  });
});
