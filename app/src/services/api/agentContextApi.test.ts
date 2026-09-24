import { beforeEach, describe, expect, it, vi } from 'vitest';

import { callCoreRpc } from '../coreRpcClient';
import { getContextBreakdown } from './agentContextApi';

vi.mock('../coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

const mockCall = vi.mocked(callCoreRpc);

const BREAKDOWN = {
  agent_id: 'orchestrator',
  model: 'reasoning-v1',
  sections: [
    { label: '## Identity', bytes: 400, est_tokens: 100 },
    { label: 'tools', bytes: 8000, est_tokens: 2000 },
  ],
  tools_bytes: 8000,
  total_est_tokens: 2100,
  context_window: 200000,
};

describe('getContextBreakdown', () => {
  beforeEach(() => mockCall.mockReset());

  it('calls agent_context_breakdown with the thread id and returns the bare response', async () => {
    mockCall.mockResolvedValueOnce(BREAKDOWN);

    const res = await getContextBreakdown('t1');

    expect(mockCall).toHaveBeenCalledWith({
      method: 'openhuman.agent_context_breakdown',
      params: { thread_id: 't1' },
    });
    expect(res).toEqual({
      sections: BREAKDOWN.sections,
      total_est_tokens: 2100,
      context_window: 200000,
    });
  });

  it('omits thread_id when there is no thread yet', async () => {
    mockCall.mockResolvedValueOnce(BREAKDOWN);

    await getContextBreakdown(null);

    expect(mockCall).toHaveBeenCalledWith({
      method: 'openhuman.agent_context_breakdown',
      params: {},
    });
  });

  it('unwraps a { result, logs } envelope', async () => {
    mockCall.mockResolvedValueOnce({ result: BREAKDOWN, logs: ['measured'] });

    const res = await getContextBreakdown('t1');

    expect(res.total_est_tokens).toBe(2100);
  });

  it('drops malformed sections and defaults missing totals to zero', async () => {
    mockCall.mockResolvedValueOnce({
      sections: [{ label: 'tools', bytes: 10, est_tokens: 3 }, { label: 7 }, null],
    });

    const res = await getContextBreakdown('t1');

    expect(res).toEqual({
      sections: [{ label: 'tools', bytes: 10, est_tokens: 3 }],
      total_est_tokens: 0,
      context_window: 0,
    });
  });

  it('rejects when the response has no sections (method missing on an older core)', async () => {
    mockCall.mockResolvedValueOnce(null);

    await expect(getContextBreakdown('t1')).rejects.toThrow(/agent_context_breakdown/);
  });

  it('propagates an RPC error', async () => {
    mockCall.mockRejectedValueOnce(new Error('Method not found'));

    await expect(getContextBreakdown('t1')).rejects.toThrow('Method not found');
  });
});
