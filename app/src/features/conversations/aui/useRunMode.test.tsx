import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { callCoreRpc } from '../../../services/coreRpcClient';
import runModeReducer from '../../../store/runModeSlice';
import { useRunMode } from './useRunMode';

vi.mock('../../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

function setup() {
  const store = configureStore({ reducer: combineReducers({ runMode: runModeReducer }) });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store}>{children}</Provider>
  );
  return { store, wrapper };
}

describe('useRunMode', () => {
  beforeEach(() => vi.mocked(callCoreRpc).mockReset());

  it('defaults to build mode with no thread', () => {
    const { wrapper } = setup();
    const { result } = renderHook(() => useRunMode(null), { wrapper });
    expect(result.current.mode).toBe('build');
  });

  it('loads the current mode via agent_get_run_mode on thread open', async () => {
    vi.mocked(callCoreRpc).mockResolvedValue({ data: { mode: 'plan' } });
    const { result } = renderHook(() => useRunMode('t1'), { wrapper: setup().wrapper });

    await waitFor(() => expect(result.current.mode).toBe('plan'));
    expect(callCoreRpc).toHaveBeenCalledWith({
      method: 'openhuman.agent_get_run_mode',
      params: { thread_id: 't1' },
    });
  });

  it('does not fetch when a value is already in the slice', async () => {
    const { store, wrapper } = setup();
    store.dispatch({ type: 'runMode/setRunMode', payload: { threadId: 't1', mode: 'plan' } });
    renderHook(() => useRunMode('t1'), { wrapper });
    await Promise.resolve();
    expect(callCoreRpc).not.toHaveBeenCalled();
  });

  it('setMode optimistically updates and calls agent_set_run_mode', async () => {
    vi.mocked(callCoreRpc).mockResolvedValue({});
    const { store, wrapper } = setup();
    const { result } = renderHook(() => useRunMode('t1'), { wrapper });

    await act(async () => {
      await result.current.setMode('plan');
    });

    expect(store.getState().runMode.byThread.t1).toBe('plan');
    expect(callCoreRpc).toHaveBeenCalledWith({
      method: 'openhuman.agent_set_run_mode',
      params: { thread_id: 't1', mode: 'plan' },
    });
  });
});
