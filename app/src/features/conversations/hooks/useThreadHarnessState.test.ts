/**
 * useThreadHarnessState — unit tests.
 *
 * The hook's job is to put the thread's settled turns behind the live one and
 * read the todo list / goal out of both, so a reopened thread keeps a goal
 * that was set several turns ago. The history RPC is mocked; the selector
 * behaviour itself is covered in `utils/harnessState.test.ts`.
 */
import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { ToolTimelineEntry } from '../../../store/chatRuntimeSlice';
import { useThreadHarnessState } from './useThreadHarnessState';

const getTurnStateHistory = vi.hoisted(() => vi.fn());
vi.mock('../../../services/api/threadApi', () => ({
  threadApi: { getTurnStateHistory: (...args: unknown[]) => getTurnStateHistory(...args) },
}));

/** A persisted tool row, as `threads_turn_state_history` returns it. */
function persisted(name: string, output: unknown) {
  return {
    id: `${name}-persisted`,
    name,
    round: 1,
    status: 'success',
    output: JSON.stringify(output),
  };
}

/** A live tool row, as the socket stream builds it. */
function live(name: string, result: unknown, seq: number): ToolTimelineEntry {
  return {
    id: `${name}-live`,
    name,
    round: 1,
    seq,
    status: 'success',
    result: JSON.stringify(result),
  };
}

const goalPayload = (status: string) => ({
  goal: {
    threadId: 't1',
    goalId: 'g1',
    objective: 'Ship the release',
    status,
    tokensUsed: 10,
    tokenBudget: 1000,
  },
  text: '',
});

const todoPayload = (statuses: string[]) => ({
  sessionId: 's1',
  todos: statuses.map((status, i) => ({ content: `step ${i + 1}`, status })),
  markdown: '',
});

const NO_LIVE_ROWS: ToolTimelineEntry[] = [];

describe('useThreadHarnessState', () => {
  beforeEach(() => {
    getTurnStateHistory.mockReset().mockResolvedValue([]);
  });

  it('reads nothing for a thread with no history and no live rows', async () => {
    const { result } = renderHook(() => useThreadHarnessState('t1', NO_LIVE_ROWS));
    await waitFor(() => expect(getTurnStateHistory).toHaveBeenCalledWith('t1'));
    expect(result.current).toEqual({ todoList: null, goal: null });
  });

  it('never calls the history RPC without a thread', () => {
    const { result } = renderHook(() => useThreadHarnessState(null, NO_LIVE_ROWS));
    expect(getTurnStateHistory).not.toHaveBeenCalled();
    expect(result.current).toEqual({ todoList: null, goal: null });
  });

  it('restores a goal set in an earlier turn (history is newest-first)', async () => {
    getTurnStateHistory.mockResolvedValue([
      // Newest turn: only worked the list.
      { toolTimeline: [persisted('todo', todoPayload(['completed', 'in_progress']))] },
      // Older turn: where the goal was set.
      { toolTimeline: [persisted('goal_set', goalPayload('active'))] },
    ]);

    const { result } = renderHook(() => useThreadHarnessState('t1', NO_LIVE_ROWS));
    await waitFor(() => expect(result.current.goal?.goalId).toBe('g1'));
    expect(result.current.goal?.status).toBe('active');
    expect(result.current.todoList?.items.map(i => i.status)).toEqual(['completed', 'in_progress']);
  });

  it('lets the live turn win over the restored history', async () => {
    getTurnStateHistory.mockResolvedValue([
      { toolTimeline: [persisted('goal_set', goalPayload('active'))] },
    ]);
    const liveRows = [live('goal_complete', goalPayload('complete'), 1)];

    const { result } = renderHook(() => useThreadHarnessState('t1', liveRows));
    await waitFor(() => expect(result.current.goal?.status).toBe('complete'));
  });

  it('still shows the live turn when the history fetch fails', async () => {
    getTurnStateHistory.mockRejectedValue(new Error('rpc down'));
    const liveRows = [live('todo', todoPayload(['in_progress']), 1)];

    const { result } = renderHook(() => useThreadHarnessState('t1', liveRows));
    await waitFor(() => expect(getTurnStateHistory).toHaveBeenCalled());
    expect(result.current.todoList?.total).toBe(1);
  });

  it('refetches and drops the previous thread state on a thread switch', async () => {
    getTurnStateHistory.mockResolvedValue([
      { toolTimeline: [persisted('goal_set', goalPayload('active'))] },
    ]);
    const { result, rerender } = renderHook(
      ({ threadId }) => useThreadHarnessState(threadId, NO_LIVE_ROWS),
      { initialProps: { threadId: 't1' } }
    );
    await waitFor(() => expect(result.current.goal?.goalId).toBe('g1'));

    getTurnStateHistory.mockResolvedValue([]);
    rerender({ threadId: 't2' });
    await waitFor(() => expect(getTurnStateHistory).toHaveBeenCalledWith('t2'));
    await waitFor(() => expect(result.current.goal).toBeNull());
  });
});
