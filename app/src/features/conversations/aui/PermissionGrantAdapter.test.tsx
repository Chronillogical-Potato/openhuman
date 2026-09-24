import { configureStore } from '@reduxjs/toolkit';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';

import { authorize } from '../../../lib/composio/composioApi';
import { callCoreRpc } from '../../../services/coreRpcClient';
import chatRuntimeReducer, { type PendingApproval } from '../../../store/chatRuntimeSlice';
import { openUrl } from '../../../utils/openUrl';
import { PermissionGrantAdapter } from './PermissionGrantAdapter';

vi.mock('../../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));
vi.mock('../../../utils/openUrl', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock('../../../lib/composio/composioApi', () => ({
  authorize: vi.fn(),
  listConnections: vi.fn().mockResolvedValue({ connections: [] }),
}));

const APPROVAL: PendingApproval = {
  requestId: 'req-1',
  toolName: 'composio_connect',
  message: 'Connect Google Drive?',
  toolkit: 'googledrive',
};

function renderAdapter() {
  const store = configureStore({ reducer: { chatRuntime: chatRuntimeReducer } });
  return render(
    <Provider store={store}>
      <PermissionGrantAdapter threadId="t-1" approval={APPROVAL} />
    </Provider>
  );
}

describe('PermissionGrantAdapter', () => {
  it('shows the capability and requester', () => {
    renderAdapter();
    expect(screen.getByText('Connect Google Drive?')).toBeInTheDocument();
    expect(screen.getByText(/composio_connect/)).toBeInTheDocument();
  });

  it('renders exactly one Connect action', () => {
    renderAdapter();
    expect(screen.getAllByRole('button', { name: /connect/i })).toHaveLength(1);
  });

  it('authorizes and opens the OAuth URL when Connect is clicked', async () => {
    vi.mocked(authorize).mockResolvedValue({ connectUrl: 'https://example.com/oauth' });
    renderAdapter();

    await userEvent.click(screen.getByRole('button', { name: /connect/i }));

    await waitFor(() => expect(authorize).toHaveBeenCalledWith('googledrive', undefined));
    await waitFor(() => expect(openUrl).toHaveBeenCalledWith('https://example.com/oauth'));
  });

  it('cancels the gate via approval_decide when Deny is clicked', async () => {
    vi.mocked(callCoreRpc).mockResolvedValue(undefined as never);
    renderAdapter();

    await userEvent.click(screen.getAllByRole('button', { name: 'Deny' })[0]!);

    await waitFor(() =>
      expect(callCoreRpc).toHaveBeenCalledWith({
        method: 'openhuman.approval_decide',
        params: { request_id: 'req-1', decision: 'deny' },
      })
    );
  });
});
