import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { callCoreRpc } from '../../../services/coreRpcClient';
import chatRuntimeReducer, { hydrateThreadUsage } from '../../../store/chatRuntimeSlice';
import { ContextUsage } from './ContextUsage';

vi.mock('../../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

const mockCall = vi.mocked(callCoreRpc);

const BREAKDOWN = {
  agent_id: 'orchestrator',
  model: 'reasoning-v1',
  sections: [
    { label: '(preamble)', bytes: 800, est_tokens: 200 },
    { label: '## Identity', bytes: 400, est_tokens: 100 },
    { label: 'tools', bytes: 8000, est_tokens: 2000 },
    { label: 'history', bytes: 20000, est_tokens: 5000 },
  ],
  tools_bytes: 8000,
  total_est_tokens: 7300,
  context_window: 100000,
};

function renderUsage(
  props: { threadId?: string | null; modelContextWindow?: number | null } = {},
  usage: { lastTurnInputTokens: number; lastTurnOutputTokens: number; contextWindow: number } = {
    lastTurnInputTokens: 40_000,
    lastTurnOutputTokens: 10_000,
    contextWindow: 200_000,
  }
) {
  const store = configureStore({ reducer: combineReducers({ chatRuntime: chatRuntimeReducer }) });
  store.dispatch(
    hydrateThreadUsage({
      threadId: 't1',
      inputTokens: 90_000,
      outputTokens: 20_000,
      cachedTokens: 30_000,
      costUsd: 0.42,
      turns: 3,
      ...usage,
    })
  );
  render(
    <Provider store={store}>
      <ContextUsage threadId={'threadId' in props ? (props.threadId ?? null) : 't1'} {...props} />
    </Provider>
  );
  return store;
}

describe('ContextUsage', () => {
  beforeEach(() => mockCall.mockReset());

  it("renders the ring from the thread's last chat_done usage against its window", () => {
    renderUsage();

    const trigger = screen.getByTestId('composer-context-usage');
    expect(trigger).toHaveAccessibleName('Context usage');
    // 40k in + 10k out of a 200k window.
    expect(trigger).toHaveTextContent('25%');
  });

  it("prefers the selected model's window over the one the last turn reported", () => {
    renderUsage({ modelContextWindow: 100_000 });

    expect(screen.getByTestId('composer-context-usage')).toHaveTextContent('50%');
  });

  it('renders at 0% before the thread has any usage', () => {
    renderUsage({ threadId: 'fresh-thread' });

    expect(screen.getByTestId('composer-context-usage')).toHaveTextContent('0%');
  });

  it('does not fetch the breakdown until the popover opens', async () => {
    mockCall.mockResolvedValue(BREAKDOWN);
    renderUsage();

    expect(mockCall).not.toHaveBeenCalled();

    await userEvent.click(screen.getByTestId('composer-context-usage'));

    expect(mockCall).toHaveBeenCalledTimes(1);
    expect(mockCall).toHaveBeenCalledWith({
      method: 'openhuman.agent_context_breakdown',
      params: { thread_id: 't1' },
    });
    const popover = await screen.findByTestId('composer-token-breakdown');
    await waitFor(() => expect(popover).toHaveTextContent('Tools'));
    expect(popover).toHaveTextContent('Conversation history');
    expect(popover).toHaveTextContent('System prompt');
    // A prompt heading is shown without its markdown hashes.
    expect(popover).toHaveTextContent('Identity');
    expect(popover).not.toHaveTextContent('## Identity');
    expect(popover).toHaveTextContent('Headroom');
    // The core's window wins inside the breakdown.
    expect(popover).toHaveTextContent('7,300 / 100,000');
  });

  it('shows an error state instead of crashing when the method is missing, and retries', async () => {
    mockCall.mockRejectedValueOnce(new Error('Method not found'));
    renderUsage();

    await userEvent.click(screen.getByTestId('composer-context-usage'));

    const popover = await screen.findByTestId('composer-token-breakdown');
    await waitFor(() => expect(popover).toHaveTextContent('Context breakdown unavailable'));
    expect(popover).not.toHaveTextContent('Method not found');

    mockCall.mockResolvedValueOnce(BREAKDOWN);
    await userEvent.click(screen.getByRole('button', { name: 'Try again' }));

    await waitFor(() => expect(popover).toHaveTextContent('Tools'));
    expect(mockCall).toHaveBeenCalledTimes(2);
  });
});
