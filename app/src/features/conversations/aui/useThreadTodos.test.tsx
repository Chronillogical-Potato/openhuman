import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { threadApi } from '../../../services/api/threadApi';
import threadTodosReducer from '../../../store/threadTodosSlice';
import { useLoadThreadTodos, useThreadTodos } from './useThreadTodos';

vi.mock('../../../services/api/threadApi', () => ({
  threadApi: { getTodos: vi.fn() },
}));

function setup() {
  const store = configureStore({ reducer: combineReducers({ threadTodos: threadTodosReducer }) });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store}>{children}</Provider>
  );
  return { store, wrapper };
}

describe('useThreadTodos', () => {
  beforeEach(() => vi.mocked(threadApi.getTodos).mockReset());

  it('returns null for a thread with no live entry', () => {
    const { wrapper } = setup();
    const { result } = renderHook(() => useThreadTodos('t1'), { wrapper });
    expect(result.current).toBeNull();
  });

  it('returns null when threadId is null', () => {
    const { wrapper } = setup();
    const { result } = renderHook(() => useThreadTodos(null), { wrapper });
    expect(result.current).toBeNull();
  });
});

describe('useLoadThreadTodos', () => {
  beforeEach(() => vi.mocked(threadApi.getTodos).mockReset());

  it('primes the slice from the RPC on thread open', async () => {
    vi.mocked(threadApi.getTodos).mockResolvedValue([{ content: 'Write tests', status: 'pending' }]);
    const { store, wrapper } = setup();
    renderHook(() => useLoadThreadTodos('t1'), { wrapper });

    await waitFor(() => expect(store.getState().threadTodos.byThread.t1).toBeDefined());
    expect(store.getState().threadTodos.byThread.t1).toEqual([
      { content: 'Write tests', status: 'pending' },
    ]);
  });

  it('leaves the slice untouched when the RPC fails (older core)', async () => {
    vi.mocked(threadApi.getTodos).mockImplementation(() => Promise.reject(new Error('no such method')));
    const { store, wrapper } = setup();
    renderHook(() => useLoadThreadTodos('t1'), { wrapper });

    await waitFor(() => expect(threadApi.getTodos).toHaveBeenCalled());
    expect(store.getState().threadTodos.byThread.t1).toBeUndefined();
  });

  it('does nothing for a null threadId', () => {
    const { wrapper } = setup();
    renderHook(() => useLoadThreadTodos(null), { wrapper });
    expect(threadApi.getTodos).not.toHaveBeenCalled();
  });
});
