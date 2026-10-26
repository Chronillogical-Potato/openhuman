import { act, screen } from '@testing-library/react';
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
    let rejectRpc: (error: Error) => void = () => {};
    hoisted.callCoreRpc.mockImplementation(
      () => new Promise((_resolve, reject) => { rejectRpc = reject; })
    );

    const Panel = (await import('../ToolPolicyDiagnosticsPanel')).default;
    renderWithProviders(<Panel />);
    await act(async () => rejectRpc(new Error('rpc transport unavailable')));

    expect(screen.getByTestId('policy-info-unavailable')).toBeInTheDocument();
    expect(screen.getByText('rpc transport unavailable')).toBeInTheDocument();
  });

  test('renders not-built state when the core does not register the method', async () => {
    let rejectRpc: (error: Error) => void = () => {};
    hoisted.callCoreRpc.mockImplementation(
      () => new Promise((_resolve, reject) => { rejectRpc = reject; })
    );

    const Panel = (await import('../ToolPolicyDiagnosticsPanel')).default;
    renderWithProviders(<Panel />);
    await act(async () => rejectRpc(new Error('method not found: security_policy_info')));

    expect(screen.getByTestId('policy-info-not-built')).toBeInTheDocument();
  });
});
