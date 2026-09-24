/**
 * Reducer-level guards for the live half of live/history parity: the rows and
 * transcript pointers the socket stream builds must have the shape — and keep
 * the slots — that a reopened thread shows, and settling a turn must be one
 * transition. The end-to-end version is `providers/__tests__/liveHistoryParity`.
 */
import type { UnknownAction } from '@reduxjs/toolkit';
import { describe, expect, it } from 'vitest';

import reducer, {
  beginInferenceTurn,
  liveTurnStarted,
  markInferenceTurnStreaming,
  registerParallelRequest,
  setInferenceStatusForThread,
  streamDeltaReceived,
  subagentSpawned,
  toolArgsDeltaReceived,
  toolCallReceived,
  turnSettled,
} from '../chatRuntimeSlice';

type ChatRuntimeState = ReturnType<typeof reducer>;

const T = 'thread-1';

function run(actions: UnknownAction[], from?: ChatRuntimeState): ChatRuntimeState {
  return actions.reduce<ChatRuntimeState>(
    (state, action) => reducer(state, action),
    from ?? reducer(undefined, { type: '@@init' })
  );
}

function pointers(state: ChatRuntimeState): string[] {
  return (state.processingByThread[T] ?? []).flatMap(item =>
    item.kind === 'toolCall' ? [item.callId] : []
  );
}

describe('toolCallReceived adopts the row a tool_args_delta minted', () => {
  it('takes over the args-delta row under the real id instead of pushing a duplicate', () => {
    const state = run([
      toolArgsDeltaReceived({ threadId: T, round: 1, delta: '{"q":1}', toolName: 'search' }),
      toolCallReceived({ threadId: T, round: 1, toolName: 'search', toolCallId: 'call-1' }),
    ]);
    const rows = state.toolTimelineByThread[T] ?? [];
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({ id: 'call-1', argsBuffer: '{"q":1}', seq: 0 });
    expect(pointers(state)).toEqual(['call-1']);
  });

  it('adopts it for an id-less call too', () => {
    const state = run([
      toolArgsDeltaReceived({ threadId: T, round: 1, delta: '{}', toolName: 'search' }),
      toolCallReceived({ threadId: T, round: 1, toolName: 'search' }),
    ]);
    expect(state.toolTimelineByThread[T]).toHaveLength(1);
    expect(pointers(state)).toHaveLength(1);
  });

  it('does not adopt a row an earlier call already owns', () => {
    const state = run([
      toolCallReceived({ threadId: T, round: 1, toolName: 'search', toolCallId: 'call-1' }),
      toolCallReceived({ threadId: T, round: 1, toolName: 'search', toolCallId: 'call-2' }),
    ]);
    expect((state.toolTimelineByThread[T] ?? []).map(row => row.id)).toEqual(['call-1', 'call-2']);
  });

  it('keeps the args the tool_call event carries, as a reload shows them', () => {
    const state = run([
      toolCallReceived({
        threadId: T,
        round: 1,
        toolName: 'search',
        toolCallId: 'call-1',
        args: { q: 'agenda' },
      }),
    ]);
    expect(state.toolTimelineByThread[T]?.[0]?.argsBuffer).toBe('{"q":"agenda"}');
  });

  it('never overwrites args that streamed', () => {
    const state = run([
      toolArgsDeltaReceived({
        threadId: T,
        round: 1,
        delta: '{"q":"streamed"}',
        toolName: 'search',
        toolCallId: 'call-1',
      }),
      toolCallReceived({
        threadId: T,
        round: 1,
        toolName: 'search',
        toolCallId: 'call-1',
        args: { q: 'event' },
      }),
    ]);
    expect(state.toolTimelineByThread[T]?.[0]?.argsBuffer).toBe('{"q":"streamed"}');
  });
});

describe('subagentSpawned promotes the spawn row in place', () => {
  const spawned = () =>
    run([
      toolCallReceived({ threadId: T, round: 1, toolName: 'shell', toolCallId: 'call-a' }),
      toolCallReceived({
        threadId: T,
        round: 1,
        toolName: 'spawn_subagent',
        toolCallId: 'call-spawn',
      }),
      toolCallReceived({ threadId: T, round: 1, toolName: 'shell', toolCallId: 'call-b' }),
      subagentSpawned({
        threadId: T,
        round: 1,
        rowId: `${T}:subagent:sub-1:researcher`,
        taskId: 'sub-1',
        agentId: 'researcher',
      }),
    ]);

  it('takes the spawn row’s slot and seq rather than moving to the end', () => {
    const rows = spawned().toolTimelineByThread[T] ?? [];
    expect(rows.map(row => row.id)).toEqual(['call-a', `${T}:subagent:sub-1:researcher`, 'call-b']);
    expect(rows.map(row => row.seq)).toEqual([0, 1, 2]);
  });

  it('re-points the transcript at the delegation row, so it is not dangling', () => {
    expect(pointers(spawned())).toEqual(['call-a', `${T}:subagent:sub-1:researcher`, 'call-b']);
  });
});

describe('turnSettled', () => {
  const live = () =>
    run([
      beginInferenceTurn({ threadId: T }),
      markInferenceTurnStreaming({ threadId: T }),
      liveTurnStarted({ threadId: T, requestId: 'req-1' }),
      setInferenceStatusForThread({
        threadId: T,
        status: { phase: 'tool_use', iteration: 1, maxIterations: 5 },
      }),
      toolCallReceived({ threadId: T, round: 1, toolName: 'shell', toolCallId: 'call-a' }),
      streamDeltaReceived({
        threadId: T,
        requestId: 'req-1',
        round: 2,
        delta: 'Done.',
        channel: 'content',
      }),
    ]);

  it('freezes the live trail under the request id, running rows settled', () => {
    const settled = reducer(live(), turnSettled({ threadId: T, requestId: 'req-1' }));
    const frozen = settled.settledTurnsByThread[T]?.['req-1'];
    expect(frozen?.timeline.map(row => [row.id, row.status])).toEqual([['call-a', 'success']]);
    expect(frozen?.transcript.map(item => item.kind)).toEqual(['toolCall', 'narration']);
  });

  it('ends the tail and everything it drew in the same transition', () => {
    const settled = reducer(live(), turnSettled({ threadId: T, requestId: 'req-1' }));
    expect(settled.inferenceTurnLifecycleByThread[T]).toBeUndefined();
    expect(settled.streamingAssistantByThread[T]).toBeUndefined();
    expect(settled.inferenceStatusByThread[T]).toBeUndefined();
    expect(settled.liveRequestIdByThread[T]).toBeUndefined();
  });

  it('falls back to the live-turn id when the event carries none', () => {
    const settled = reducer(live(), turnSettled({ threadId: T }));
    expect(settled.settledTurnsByThread[T]?.['req-1']).toBeDefined();
  });

  it('keeps a bounded number of frozen turns per thread', () => {
    let state = live();
    for (let n = 0; n < 30; n += 1) {
      state = run(
        [
          toolCallReceived({ threadId: T, round: 1, toolName: 'shell', toolCallId: `c-${n}` }),
          turnSettled({ threadId: T, requestId: `req-${n}` }),
        ],
        state
      );
    }
    const kept = Object.keys(state.settledTurnsByThread[T] ?? {});
    expect(kept).toHaveLength(20);
    expect(kept.at(-1)).toBe('req-29');
  });
});

describe('turn boundaries', () => {
  it('a newer live turn is not torn down by the previous turn settling late', () => {
    const state = run([
      beginInferenceTurn({ threadId: T }),
      markInferenceTurnStreaming({ threadId: T }),
      liveTurnStarted({ threadId: T, requestId: 'req-2' }),
      toolCallReceived({ threadId: T, round: 1, toolName: 'shell', toolCallId: 'call-2' }),
      turnSettled({ threadId: T, requestId: 'req-1' }),
    ]);
    expect(state.inferenceTurnLifecycleByThread[T]).toBe('streaming');
    expect(state.liveRequestIdByThread[T]).toBe('req-2');
    expect(state.toolTimelineByThread[T]?.[0]?.status).toBe('running');
    expect(state.settledTurnsByThread[T]?.['req-1']).toBeUndefined();
  });

  it('a new send starts from an empty transcript, not the last turn’s', () => {
    const state = run([
      streamDeltaReceived({
        threadId: T,
        requestId: 'req-1',
        round: 1,
        delta: 'Last turn said this.',
        channel: 'content',
      }),
      turnSettled({ threadId: T, requestId: 'req-1' }),
      beginInferenceTurn({ threadId: T }),
    ]);
    expect(state.processingByThread[T]).toBeUndefined();
    // …while the settled turn keeps the trail it rendered with.
    expect(state.settledTurnsByThread[T]?.['req-1']?.transcript).toHaveLength(1);
  });
});

describe('liveTurnStarted', () => {
  it('ignores a parallel (forked) request, which never owns the tail', () => {
    const state = run([
      registerParallelRequest({ threadId: T, requestId: 'fork' }),
      liveTurnStarted({ threadId: T, requestId: 'fork' }),
    ]);
    expect(state.liveRequestIdByThread[T]).toBeUndefined();
  });
});
