import { screen, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { renderWithProviders } from '../../../../test/test-utils';

const hoisted = vi.hoisted(() => ({ callCoreRpc: vi.fn() }));

vi.mock('../../../../services/coreRpcClient', () => ({
  callCoreRpc: (...args: unknown[]) => hoisted.callCoreRpc(...args),
}));

vi.mock('../../hooks/useSettingsNavigation', () => ({
  useSettingsNavigation: () => ({ navigateBack: vi.fn(), breadcrumbs: [] }),
}));

describe('ToolPolicyDiagnosticsPanel', () => {
  test('renders active security policy from core RPC', async () => {
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

    await waitFor(() => expect(screen.getByText('supervised')).toBeInTheDocument());
    expect(hoisted.callCoreRpc).toHaveBeenCalledWith(
      expect.objectContaining({ method: 'openhuman.security_policy_info' })
    );
  });

  test('renders unavailable card when the RPC throws', async () => {
    hoisted.callCoreRpc.mockReset();
    hoisted.callCoreRpc.mockRejectedValue(new Error('rpc transport unavailable'));

    const Panel = (await import('../ToolPolicyDiagnosticsPanel')).default;
    renderWithProviders(<Panel />);

    await waitFor(() => {
      expect(screen.getByText(/Diagnostics unavailable/i)).toBeInTheDocument();
    });
    expect(screen.getByText(/rpc transport unavailable/i)).toBeInTheDocument();
  });

  test('shows a not-built state when the security method is missing', async () => {
    hoisted.callCoreRpc.mockReset();
    hoisted.callCoreRpc.mockRejectedValue(new Error('method not found: security_policy_info'));

    const Panel = (await import('../ToolPolicyDiagnosticsPanel')).default;
    renderWithProviders(<Panel />);

    await waitFor(() => expect(screen.getByText(/not built/i)).toBeInTheDocument());
  });

  test('renders mcp_allowlists per-server rows when populated', async () => {
    hoisted.callCoreRpc.mockReset();
    hoisted.callCoreRpc.mockResolvedValue({
      total_tools: 4,
      enabled_tools: 4,
      mcp_stdio_tools: 1,
      json_rpc_tools: 3,
      possible_write_surfaces: [],
      policy_surfaces: [],
      posture: {
        autonomy_level: 'full',
        workspace_only: false,
        max_actions_per_hour: 0,
        require_approval_for_medium_risk: false,
        block_high_risk_commands: false,
      },
      mcp_allowlists: {
        enabled: true,
        server_count: 2,
        enabled_server_count: 2,
        servers: [
          { name: 'fs', allowed_tools_count: 5, disallowed_tools_count: 1 },
          { name: 'shell', allowed_tools_count: 2, disallowed_tools_count: 3 },
        ],
      },
      mcp_write_audit: { enabled: true, recent_rows: 0, last_error: null },
      recent_denials: [],
    });

    const Panel = (await import('../ToolPolicyDiagnosticsPanel')).default;
    renderWithProviders(<Panel />);

    await waitFor(() => {
      expect(screen.getByText('fs')).toBeInTheDocument();
    });
    expect(screen.getByText('shell')).toBeInTheDocument();
  });

  test('renders mcp_write_audit last_error when present', async () => {
    hoisted.callCoreRpc.mockReset();
    hoisted.callCoreRpc.mockResolvedValue({
      total_tools: 1,
      enabled_tools: 1,
      mcp_stdio_tools: 0,
      json_rpc_tools: 1,
      possible_write_surfaces: [],
      policy_surfaces: [],
      posture: {
        autonomy_level: 'readonly',
        workspace_only: true,
        max_actions_per_hour: 1,
        require_approval_for_medium_risk: true,
        block_high_risk_commands: true,
      },
      mcp_allowlists: { enabled: false, server_count: 0, enabled_server_count: 0, servers: [] },
      mcp_write_audit: {
        enabled: true,
        recent_rows: 0,
        last_error: 'SQLITE_BUSY: database is locked',
      },
      recent_denials: [],
    });

    const Panel = (await import('../ToolPolicyDiagnosticsPanel')).default;
    renderWithProviders(<Panel />);

    await waitFor(() => {
      expect(screen.getByText(/SQLITE_BUSY/i)).toBeInTheDocument();
    });
  });
});
