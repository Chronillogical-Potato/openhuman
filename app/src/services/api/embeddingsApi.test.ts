import { beforeEach, describe, expect, it, vi } from 'vitest';

import { callCoreRpc } from '../coreRpcClient';
import { updateEmbeddingsSettings } from './embeddingsApi';

vi.mock('../coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

const mockCall = vi.mocked(callCoreRpc);

describe('updateEmbeddingsSettings', () => {
  beforeEach(() => mockCall.mockReset());

  it('omits the retired confirmation flag from the core RPC', async () => {
    mockCall.mockResolvedValueOnce({ result: { provider: 'none' } });

    await updateEmbeddingsSettings({ provider: 'none', confirm_wipe: true });

    expect(mockCall).toHaveBeenCalledWith({
      method: 'openhuman.embeddings_update_settings',
      params: { provider: 'none' },
    });
  });
});
