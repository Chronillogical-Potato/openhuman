import { describe, expect, it } from 'vitest';

import reducer, { clearThreadGoal, setThreadGoal, type ThreadGoalState } from './threadGoalSlice';

const initial: ThreadGoalState = { byThread: {} };

const goal = {
  goal_id: 'g1',
  objective: 'Ship the feature',
  status: 'active' as const,
  tokens_used: 100,
  token_budget: 1000,
};

describe('threadGoalSlice', () => {
  it('sets the goal for a thread', () => {
    const next = reducer(initial, setThreadGoal({ threadId: 't1', goal }));
    expect(next.byThread.t1).toEqual(goal);
  });

  it('sets null when the update payload is null', () => {
    const withGoal = reducer(initial, setThreadGoal({ threadId: 't1', goal }));
    const next = reducer(withGoal, setThreadGoal({ threadId: 't1', goal: null }));
    expect(next.byThread.t1).toBeNull();
  });

  it('clears a thread to null', () => {
    const withGoal = reducer(initial, setThreadGoal({ threadId: 't1', goal }));
    const cleared = reducer(withGoal, clearThreadGoal({ threadId: 't1' }));
    expect(cleared.byThread.t1).toBeNull();
  });

  it('keeps other threads untouched', () => {
    const withT1 = reducer(initial, setThreadGoal({ threadId: 't1', goal }));
    const withT2 = reducer(
      withT1,
      setThreadGoal({ threadId: 't2', goal: { ...goal, goal_id: 'g2' } })
    );
    expect(withT2.byThread.t1?.goal_id).toBe('g1');
    expect(withT2.byThread.t2?.goal_id).toBe('g2');
  });
});
