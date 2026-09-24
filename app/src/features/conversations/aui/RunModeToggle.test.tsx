import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { callCoreRpc } from '../../../services/coreRpcClient';
import runModeReducer from '../../../store/runModeSlice';
import { RunModeToggle } from './RunModeToggle';

vi.mock('../../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

function renderToggle() {
  const store = configureStore({ reducer: combineReducers({ runMode: runModeReducer }) });
  render(
    <Provider store={store}>
      <RunModeToggle threadId="t1" />
    </Provider>
  );
  return store;
}

describe('RunModeToggle', () => {
  beforeEach(() => vi.mocked(callCoreRpc).mockReset().mockResolvedValue({}));

  it('shows the build label by default', () => {
    renderToggle();
    expect(screen.getByTestId('run-mode-toggle')).toHaveAttribute('data-run-mode', 'build');
  });

  it('flips to plan mode on click and calls agent_set_run_mode', async () => {
    const store = renderToggle();
    await userEvent.click(screen.getByTestId('run-mode-toggle'));

    expect(store.getState().runMode.byThread.t1).toBe('plan');
    expect(callCoreRpc).toHaveBeenCalledWith({
      method: 'openhuman.agent_set_run_mode',
      params: { thread_id: 't1', mode: 'plan' },
    });
  });
});
