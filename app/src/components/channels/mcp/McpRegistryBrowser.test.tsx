import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import McpRegistryBrowser, { deriveRepoUrl, serverPageUrl } from './McpRegistryBrowser';

const mockRegistrySearch = vi.fn();
const mockOpenUrl = vi.fn();

vi.mock('../../../services/api/mcpClientsApi', () => ({
  mcpClientsApi: { registrySearch: (...args: unknown[]) => mockRegistrySearch(...args) },
}));

vi.mock('../../../utils/openUrl', () => ({
  openUrl: (...args: unknown[]) => mockOpenUrl(...args),
}));

const SERVERS = [
  {
    qualified_name: 'io.github.acme/echo',
    display_name: 'Echo',
    description: 'Echoes things.',
    source: 'mcp_official',
    official: true,
  },
  {
    qualified_name: 'vendor/hosted-thing',
    display_name: 'Hosted Thing',
    is_deployed: true,
    website_url: 'https://hosted.example',
    source: 'mcp_official',
  },
  { qualified_name: 'installed/already', display_name: 'Already Declared', source: 'smithery' },
];

describe('serverPageUrl', () => {
  it('prefers the declared website, then the repository, then the directory listing', () => {
    expect(serverPageUrl(SERVERS[1])).toBe('https://hosted.example');
    expect(serverPageUrl(SERVERS[0])).toBe('https://github.com/acme/echo');
    expect(serverPageUrl(SERVERS[2])).toBe('https://smithery.ai/server/installed/already');
    expect(serverPageUrl({ qualified_name: 'com.vendor/x', display_name: 'X' })).toBe(
      'https://registry.modelcontextprotocol.io/?search=com.vendor%2Fx'
    );
  });

  it('derives repository urls only from code-host slugs', () => {
    expect(deriveRepoUrl('io.gitlab.acme/echo')).toBe('https://gitlab.com/acme/echo');
    expect(deriveRepoUrl('com.vendor/x')).toBeNull();
    expect(deriveRepoUrl('noslash')).toBeNull();
  });
});

describe('McpRegistryBrowser', () => {
  beforeEach(() => {
    mockRegistrySearch.mockReset();
    mockOpenUrl.mockReset();
    mockOpenUrl.mockResolvedValue(undefined);
    mockRegistrySearch.mockResolvedValue({ servers: SERVERS, page: 1, total_pages: 1 });
  });

  it('lists the directory minus already-declared servers, and opens a row in the browser', async () => {
    render(<McpRegistryBrowser installedNames={new Set(['installed/already'])} />);
    const rows = await screen.findAllByTestId('mcp-registry-row');
    expect(rows).toHaveLength(2);
    expect(screen.queryByText('Already Declared')).not.toBeInTheDocument();
    // No install control anywhere: the row's action is to open the page.
    expect(screen.queryByText('Install')).not.toBeInTheDocument();
    expect(screen.getAllByText(/Open page/)).toHaveLength(2);

    fireEvent.click(screen.getByRole('button', { name: 'Open the page for Echo' }));
    expect(mockOpenUrl).toHaveBeenCalledWith('https://github.com/acme/echo');
  });

  it('opens a row from the keyboard but not from a nested link', async () => {
    render(<McpRegistryBrowser installedNames={new Set()} />);
    await screen.findAllByTestId('mcp-registry-row');
    const row = screen.getByRole('button', { name: 'Open the page for Hosted Thing' });
    fireEvent.keyDown(row, { key: 'Enter' });
    expect(mockOpenUrl).toHaveBeenCalledWith('https://hosted.example');

    mockOpenUrl.mockClear();
    fireEvent.click(screen.getByRole('button', { name: 'Repository' }));
    // The nested link opens its own target only, not the row's page as well.
    expect(mockOpenUrl).toHaveBeenCalledTimes(1);
    expect(mockOpenUrl).toHaveBeenCalledWith('https://github.com/acme/echo');
  });

  it('shows a directory outage with retry instead of an empty result', async () => {
    mockRegistrySearch.mockRejectedValueOnce(new Error('registry down'));
    render(<McpRegistryBrowser installedNames={new Set()} />);
    await screen.findByTestId('mcp-catalog-error');
    expect(screen.queryByTestId('mcp-catalog-empty')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    await waitFor(() => expect(screen.getAllByTestId('mcp-registry-row')).toHaveLength(3));
  });

  it('re-queries the directory with the transport filter', async () => {
    render(<McpRegistryBrowser installedNames={new Set()} />);
    await screen.findAllByTestId('mcp-registry-row');
    fireEvent.click(screen.getByRole('button', { name: 'Hosted' }));
    await waitFor(() =>
      expect(mockRegistrySearch).toHaveBeenLastCalledWith(
        expect.objectContaining({ transport: 'hosted', page: 1 })
      )
    );
  });
});
