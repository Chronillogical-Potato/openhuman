import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { threadApi } from '../../../services/api/threadApi';
import threadGoalReducer from '../../../store/threadGoalSlice';
import { formatTokens, useLoadThreadGoal, useThreadGoal } from './useThreadGoal';

vi.mock('../../../services/api/threadApi', () => ({
  threadApi: { getGoal: vi.fn() },
}));

function setup() {
  const store = configureStore({ reducer: combineReducers({ threadGoal: threadGoalReducer }) });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store}>{children}</Provider>
  );
  return { store, wrapper };
}

describe('formatTokens', () => {
  it('keeps small counts exact', () => {
    expect(formatTokens(42)).toBe('42');
  });
  it('formats thousands', () => {
    expect(formatTokens(1200)).toBe('1.2k');
  });
  it('formats millions', () => {
    expect(formatTokens(2_500_000)).toBe('2.5M');
  });
});

describe('useThreadGoal', () => {
  it('returns null for a thread with no goal loaded', () => {
    const { wrapper } = setup();
    const { result } = renderHook(() => useThreadGoal('t1'), { wrapper });
    expect(result.current).toBeNull();
  });
});

describe('useLoadThreadGoal', () => {
  beforeEach(() => vi.mocked(threadApi.getGoal).mockReset());

  it('primes the slice from the RPC on thread open', async () => {
    const goal = {
      goal_id: 'g1',
      objective: 'Ship it',
      status: 'active' as const,
      tokens_used: 10,
    };
    vi.mocked(threadApi.getGoal).mockResolvedValue(goal);
    const { store, wrapper } = setup();
    renderHook(() => useLoadThreadGoal('t1'), { wrapper });

    await waitFor(() => expect(store.getState().threadGoal.byThread.t1).toEqual(goal));
  });

  it('leaves the slice untouched when the RPC fails', async () => {
    vi.mocked(threadApi.getGoal).mockImplementation(() => Promise.reject(new Error('no such method')));
    const { store, wrapper } = setup();
    renderHook(() => useLoadThreadGoal('t1'), { wrapper });

    await waitFor(() => expect(threadApi.getGoal).toHaveBeenCalled());
    await new Promise(resolve => setTimeout(resolve, 10));
    expect(store.getState().threadGoal.byThread.t1).toBeUndefined();
  });
});
