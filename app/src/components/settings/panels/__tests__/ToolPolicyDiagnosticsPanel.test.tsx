import { screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { renderWithProviders } from '../../../../test/test-utils';

const hoisted = vi.hoisted(() => ({ callCoreRpc: vi.fn() }));

vi.mock('../../../../services/coreRpcClient', () => ({
  callCoreRpc: (...args: unknown[]) => hoisted.callCoreRpc(...args),
}));

vi.mock('../../hooks/useSettingsNavigation', () => ({
  useSettingsNavigation: () => ({ navigateBack: vi.fn(), breadcrumbs: [] }),
}));

describe('ToolPolicyDiagnosticsPanel', () => {
  beforeEach(() => hoisted.callCoreRpc.mockReset());

  test('renders active policy from security_policy_info', async () => {
    hoisted.callCoreRpc.mockResolvedValue({
      autonomy: 'supervised',
      workspace_only: true,
      allowed_commands: ['ls'],
      max_actions_per_hour: 123,
      require_approval_for_medium_risk: true,
      block_high_risk_commands: true,
    });

    const Panel = (await import('../ToolPolicyDiagnosticsPanel')).default;
    renderWithProviders(<Panel />);

    expect(await screen.findByTestId('policy-info-ready')).toBeInTheDocument();
    expect(screen.getByText('supervised')).toBeInTheDocument();
    expect(screen.getByText('ls')).toBeInTheDocument();
    expect(hoisted.callCoreRpc).toHaveBeenCalledWith(
      expect.objectContaining({ method: 'openhuman.security_policy_info' })
    );
  });

  test('renders unavailable state when the RPC fails', async () => {
    hoisted.callCoreRpc.mockRejectedValue(new Error('rpc transport unavailable'));

    const Panel = (await import('../ToolPolicyDiagnosticsPanel')).default;
    renderWithProviders(<Panel />);

    expect(await screen.findByTestId('policy-info-unavailable')).toBeInTheDocument();
    expect(screen.getByText('rpc transport unavailable')).toBeInTheDocument();
  });

  test('renders not-built state when the core does not register the method', async () => {
    hoisted.callCoreRpc.mockRejectedValue(new Error('method not found: security_policy_info'));

    const Panel = (await import('../ToolPolicyDiagnosticsPanel')).default;
    renderWithProviders(<Panel />);

    await waitFor(() => expect(screen.getByTestId('policy-info-not-built')).toBeInTheDocument());
  });
});
