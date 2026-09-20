import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import McpServersTab from './McpServersTab';

const mockInstalledList = vi.fn();
const mockStatus = vi.fn();
const mockRegistrySearch = vi.fn();
const mockConfigGet = vi.fn();
const mockConfigSet = vi.fn();

vi.mock('../../../services/api/mcpClientsApi', () => ({
  mcpClientsApi: {
    installedList: (...args: unknown[]) => mockInstalledList(...args),
    status: (...args: unknown[]) => mockStatus(...args),
    registrySearch: (...args: unknown[]) => mockRegistrySearch(...args),
    configGet: (...args: unknown[]) => mockConfigGet(...args),
    configSet: (...args: unknown[]) => mockConfigSet(...args),
    connect: vi.fn(),
    disconnect: vi.fn(),
    uninstall: vi.fn(),
    updateEnv: vi.fn(),
    detectAuth: vi.fn().mockResolvedValue({ kind: 'none', grant_types: [] }),
    registryGet: vi.fn().mockResolvedValue({ connections: [], required_env_keys: [] }),
  },
}));

vi.mock('../../../utils/openUrl', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));

const LOCAL = {
  server_id: 'srv-local',
  qualified_name: 'echo',
  display_name: 'echo',
  command_kind: 'node' as const,
  command: 'npx',
  args: ['-y', 'echo'],
  env_keys: [],
  installed_at: 1,
  transport: { kind: 'stdio' as const },
  enabled: true,
};
const HOSTED = {
  server_id: 'srv-hosted',
  qualified_name: 'hosted',
  display_name: 'hosted',
  command_kind: 'node' as const,
  command: '',
  args: [],
  env_keys: ['Authorization'],
  installed_at: 2,
  transport: { kind: 'http_remote' as const, url: 'https://h.test/mcp' },
  enabled: true,
};

describe('McpServersTab', () => {
  beforeEach(() => {
    mockInstalledList.mockReset();
    mockStatus.mockReset();
    mockRegistrySearch.mockReset();
    mockConfigGet.mockReset();
    mockConfigSet.mockReset();
    mockInstalledList.mockResolvedValue([LOCAL, HOSTED]);
    mockStatus.mockResolvedValue([{ server_id: 'srv-local', status: 'connected' }]);
    mockRegistrySearch.mockResolvedValue({ servers: [], page: 1, total_pages: 1 });
    mockConfigGet.mockResolvedValue({ mcpServers: {} });
  });

  it('opens on the server rows with the three notations as tabs', async () => {
    render(<McpServersTab />);
    await screen.findByTestId('mcp-servers-section');
    expect(screen.getByRole('tab', { name: 'Servers (2)' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'mcp.json' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Registry' })).toBeInTheDocument();

    const rows = screen.getAllByTestId('mcp-installed-row');
    expect(rows).toHaveLength(2);
    // The dial column says how each one runs, not where it was found.
    expect(screen.getByText('npx -y echo')).toBeInTheDocument();
    expect(screen.getByText('h.test')).toBeInTheDocument();
    // Nothing from the directory sits among the user's own rows.
    expect(mockRegistrySearch).not.toHaveBeenCalled();
  });

  it('switches to the document and the directory, each rendered on demand', async () => {
    render(<McpServersTab />);
    await screen.findByTestId('mcp-servers-section');

    fireEvent.click(screen.getByRole('tab', { name: 'mcp.json' }));
    await screen.findByTestId('mcp-json-editor');
    expect(mockConfigGet).toHaveBeenCalled();
    expect(screen.queryByTestId('mcp-servers-section')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: 'Registry' }));
    await screen.findByTestId('mcp-registry-browser');
    await waitFor(() => expect(mockRegistrySearch).toHaveBeenCalled());
  });

  it('honours an initial tab', async () => {
    render(<McpServersTab initialTab="registry" />);
    await screen.findByTestId('mcp-registry-browser');
  });

  it('re-reads the rows after the document is saved', async () => {
    mockConfigGet.mockResolvedValue({ mcpServers: { echo: { command: 'npx' } } });
    mockConfigSet.mockResolvedValue({
      mcpServers: {},
      added: [],
      updated: [],
      removed: ['echo', 'hosted'],
    });
    render(<McpServersTab />);
    await screen.findByTestId('mcp-servers-section');
    expect(mockInstalledList).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('tab', { name: 'mcp.json' }));
    await screen.findByTestId('mcp-json-editor');
    mockInstalledList.mockResolvedValue([]);
    fireEvent.change(screen.getByTestId('mcp-json-textarea'), {
      target: { value: '{ "mcpServers": {} }' },
    });
    fireEvent.click(screen.getByTestId('mcp-json-save'));

    await waitFor(() => expect(mockInstalledList).toHaveBeenCalledTimes(2));
    fireEvent.click(screen.getByRole('tab', { name: 'Servers (0)' }));
    await screen.findByTestId('mcp-installed-empty');
  });

  it('opens a row into its detail view and comes back', async () => {
    render(<McpServersTab />);
    await screen.findByTestId('mcp-servers-section');
    fireEvent.click(screen.getByRole('button', { name: 'View details for echo' }));
    expect(await screen.findByRole('button', { name: 'Back to servers' })).toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: 'mcp.json' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Back to servers' }));
    await screen.findByTestId('mcp-servers-section');
  });

  it('routes an empty list to the document and the directory', async () => {
    mockInstalledList.mockResolvedValue([]);
    mockStatus.mockResolvedValue([]);
    render(<McpServersTab />);
    await screen.findByTestId('mcp-installed-empty');
    fireEvent.click(screen.getByRole('button', { name: 'Browse the registry' }));
    await screen.findByTestId('mcp-registry-browser');
  });
});
