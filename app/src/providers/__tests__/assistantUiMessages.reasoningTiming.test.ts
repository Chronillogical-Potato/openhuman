import { describe, expect, it } from 'vitest';

import type { ThreadMessage } from '../../types/thread';
import { reasoningPart, streamingTailMessage, toThreadMessageLike } from '../assistantUiMessages';

/**
 * Reasoning parts carry their block's timing to the thread's reasoning panel
 * under `providerMetadata.openhuman`. Without it the panel cannot show
 * "Thought for Ns" (the regression this guards against).
 */

const agentMsg: ThreadMessage = {
  id: 'm1',
  content: 'answer',
  type: 'text',
  extraMetadata: {},
  sender: 'agent',
  createdAt: '2026-09-24T00:00:00Z',
} as ThreadMessage;

describe('reasoningPart', () => {
  it('attaches timing when known', () => {
    expect(reasoningPart('x', 1_000, 5_000)).toEqual({
      type: 'reasoning',
      text: 'x',
      providerMetadata: { openhuman: { startedAt: 1_000, endedAt: 5_000 } },
    });
  });

  it('stays a bare part when no timing is known (legacy rows)', () => {
    expect(reasoningPart('x', undefined, undefined)).toEqual({ type: 'reasoning', text: 'x' });
  });

  it('keeps a start with no end yet', () => {
    expect(reasoningPart('x', 1_000, undefined)).toEqual({
      type: 'reasoning',
      text: 'x',
      providerMetadata: { openhuman: { startedAt: 1_000 } },
    });
  });
});

describe('timing on projected reasoning parts', () => {
  it('settled messages carry each transcript block’s timing', () => {
    const like = toThreadMessageLike(
      agentMsg,
      [],
      [{ kind: 'thinking', round: 1, seq: 0, text: 'plan', startedAt: 10, endedAt: 20 }]
    );
    expect(like.content).toEqual([
      reasoningPart('plan', 10, 20),
      { type: 'text', text: 'answer' },
    ]);
  });

  it('the live tail carries the streaming thinking span', () => {
    const tail = streamingTailMessage({
      requestId: 'r',
      content: '',
      thinking: 'still thinking',
      thinkingStartedAt: 1_000,
      thinkingEndedAt: 3_000,
    });
    expect(tail?.content).toEqual([reasoningPart('still thinking', 1_000, 3_000)]);
  });

  it('the live tail uses transcript timing once the block is recorded', () => {
    const tail = streamingTailMessage(
      { requestId: 'r', content: '', thinking: 'recorded', thinkingStartedAt: 1 },
      [],
      [{ kind: 'thinking', round: 1, seq: 0, text: 'recorded', startedAt: 5, endedAt: 9 }]
    );
    expect(tail?.content).toEqual([reasoningPart('recorded', 5, 9)]);
  });
});
