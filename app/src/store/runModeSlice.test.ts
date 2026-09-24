import { describe, expect, it } from 'vitest';

import reducer, { setRunMode, type RunModeState } from './runModeSlice';

const initial: RunModeState = { byThread: {} };

describe('runModeSlice', () => {
  it('sets the mode for a thread', () => {
    const next = reducer(initial, setRunMode({ threadId: 't1', mode: 'plan' }));
    expect(next.byThread.t1).toBe('plan');
  });

  it('flips a thread from plan to build', () => {
    const withPlan = reducer(initial, setRunMode({ threadId: 't1', mode: 'plan' }));
    const withBuild = reducer(withPlan, setRunMode({ threadId: 't1', mode: 'build' }));
    expect(withBuild.byThread.t1).toBe('build');
  });

  it('keeps other threads untouched', () => {
    const withT1 = reducer(initial, setRunMode({ threadId: 't1', mode: 'plan' }));
    const withT2 = reducer(withT1, setRunMode({ threadId: 't2', mode: 'build' }));
    expect(withT2.byThread.t1).toBe('plan');
    expect(withT2.byThread.t2).toBe('build');
  });
});
