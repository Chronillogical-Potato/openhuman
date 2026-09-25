import { configureStore } from '@reduxjs/toolkit';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { callCoreRpc } from '../../services/coreRpcClient';
import UsageLogPanel from './UsageLogPanel';

vi.mock('../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));
const mockedCall = vi.mocked(callCoreRpc);

const records = [
  {
    id: 'one',
    timestamp: '2026-09-24T12:00:00Z',
    session_id: 'session-one',
    model: 'alpha',
    provider: 'openrouter',
    category: 'Chat',
    input_tokens: 100,
    output_tokens: 50,
    total_tokens: 150,
    cached_input_tokens: 0,
    cache_creation_tokens: 0,
    reasoning_tokens: 0,
    cost_usd: 1,
    cost_source: 'estimated',
  },
  {
    id: 'two',
    timestamp: '2026-09-24T11:00:00Z',
    session_id: 'session-two',
    model: 'beta',
    provider: 'anthropic',
    category: 'Voice',
    input_tokens: 200,
    output_tokens: 20,
    total_tokens: 220,
    cached_input_tokens: 0,
    cache_creation_tokens: 0,
    reasoning_tokens: 0,
    cost_usd: 2,
    cost_source: 'provider_charged',
  },
];

function renderPanel() {
  const store = configureStore({ reducer: { locale: (state = { current: 'en' }) => state } });
  return render(
    <Provider store={store}>
      <UsageLogPanel />
    </Provider>
  );
}

describe('<UsageLogPanel />', () => {
  beforeEach(() => {
    mockedCall.mockReset();
    mockedCall.mockResolvedValue({
      records,
      by_category: [],
      total_cost_usd: 3,
      total_tokens: 370,
      request_count: 2,
      currency: 'USD',
      days: 30,
      limit: 1000,
    } as never);
  });

  it('filters loaded rows and labels the bounded result', async () => {
    renderPanel();
    await screen.findByText('alpha');
    expect(screen.getByText('2 of 2 loaded records • $3.00')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Category'), { target: { value: 'Voice' } });
    expect(screen.queryByText('alpha')).not.toBeInTheDocument();
    expect(screen.getByText('beta')).toBeInTheDocument();
    expect(screen.getByText('1 of 2 loaded records • $2.00')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Model or session'), { target: { value: 'missing' } });
    expect(screen.getByText('No usage records found for this period.')).toBeInTheDocument();
  });

  it('requests the chosen period from the core', async () => {
    renderPanel();
    await screen.findByText('alpha');
    fireEvent.change(screen.getByLabelText('Period'), { target: { value: '7' } });
    await waitFor(() =>
      expect(mockedCall).toHaveBeenCalledWith(
        expect.objectContaining({
          method: 'openhuman.cost_get_usage_log',
          params: { days: 7, limit: 1000 },
        })
      )
    );
  });

  it('filters by provider and whether cost was reported by the provider', async () => {
    renderPanel();
    await screen.findByText('alpha');

    fireEvent.change(screen.getByLabelText('Provider'), { target: { value: 'anthropic' } });
    expect(screen.queryByText('alpha')).not.toBeInTheDocument();
    expect(screen.getByText('beta')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Cost source'), { target: { value: 'estimated' } });
    expect(screen.getByText('No usage records found for this period.')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Cost source'), {
      target: { value: 'provider_charged' },
    });
    expect(screen.getByText('beta')).toBeInTheDocument();
  });
});
