import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import McpServerForm, { splitArgs } from './McpServerForm';

const mockConfigGet = vi.fn();
const mockConfigSet = vi.fn();

vi.mock('../../../services/api/mcpClientsApi', () => ({
  mcpClientsApi: {
    configGet: (...args: unknown[]) => mockConfigGet(...args),
    configSet: (...args: unknown[]) => mockConfigSet(...args),
  },
}));

const EXISTING_DOC = { mcpServers: { other: { command: 'uvx', args: ['thing'] } } };

const select = (testId: string, value: string) =>
  fireEvent.change(screen.getByTestId(testId), { target: { value } });

describe('splitArgs', () => {
  it('splits on whitespace and honours quotes', () => {
    expect(splitArgs('-y @x/y "a path with spaces" \'single\'')).toEqual([
      '-y',
      '@x/y',
      'a path with spaces',
      'single',
    ]);
    expect(splitArgs('   ')).toEqual([]);
  });
});

describe('McpServerForm', () => {
  beforeEach(() => {
    mockConfigGet.mockReset();
    mockConfigSet.mockReset();
    mockConfigGet.mockResolvedValue(EXISTING_DOC);
    mockConfigSet.mockResolvedValue({ mcpServers: {}, added: [], updated: [], removed: [] });
  });

  it('adds a local command with its arguments and env, merged into the document', async () => {
    const onSaved = vi.fn();
    render(<McpServerForm onClose={() => {}} onSaved={onSaved} />);
    expect(screen.getByTestId('mcp-form-save')).toBeDisabled();

    fireEvent.change(screen.getByTestId('mcp-form-name'), { target: { value: 'github' } });
    fireEvent.change(screen.getByTestId('mcp-form-command'), { target: { value: 'npx' } });
    fireEvent.change(screen.getByTestId('mcp-form-args'), {
      target: { value: '-y @modelcontextprotocol/server-github' },
    });
    fireEvent.change(screen.getByLabelText('Variable name'), {
      target: { value: 'GITHUB_TOKEN' },
    });
    fireEvent.change(screen.getByLabelText('Value'), { target: { value: 'ghp_x' } });
    expect(screen.getByTestId('mcp-form-save')).toBeEnabled();
    fireEvent.click(screen.getByTestId('mcp-form-save'));

    await waitFor(() => expect(onSaved).toHaveBeenCalledWith('github', { signIn: false }));
    expect(mockConfigSet).toHaveBeenCalledWith({
      mcpServers: {
        other: { command: 'uvx', args: ['thing'] },
        github: {
          command: 'npx',
          args: ['-y', '@modelcontextprotocol/server-github'],
          env: { GITHUB_TOKEN: 'ghp_x' },
        },
      },
    });
  });

  it('adds a remote URL with a bearer token as an Authorization header', async () => {
    const onSaved = vi.fn();
    render(<McpServerForm onClose={() => {}} onSaved={onSaved} />);
    fireEvent.change(screen.getByTestId('mcp-form-name'), { target: { value: 'linear' } });
    select('mcp-form-transport', 'http');
    fireEvent.change(screen.getByTestId('mcp-form-url'), {
      target: { value: 'https://mcp.linear.app/mcp' },
    });
    select('mcp-form-auth', 'bearer');
    fireEvent.change(screen.getByTestId('mcp-form-token'), { target: { value: 'lin_abc' } });
    fireEvent.click(screen.getByTestId('mcp-form-save'));

    await waitFor(() => expect(onSaved).toHaveBeenCalledWith('linear', { signIn: false }));
    expect(mockConfigSet.mock.calls[0][0].mcpServers.linear).toEqual({
      url: 'https://mcp.linear.app/mcp',
      headers: { Authorization: 'Bearer lin_abc' },
    });
  });

  it('saves a remote URL with browser sign-in and asks the caller to sign in', async () => {
    const onSaved = vi.fn();
    render(<McpServerForm onClose={() => {}} onSaved={onSaved} />);
    fireEvent.change(screen.getByTestId('mcp-form-name'), { target: { value: 'notion' } });
    select('mcp-form-transport', 'http');
    fireEvent.change(screen.getByTestId('mcp-form-url'), {
      target: { value: 'https://mcp.notion.com/mcp' },
    });
    select('mcp-form-auth', 'oauth');
    expect(screen.getByTestId('mcp-form-save')).toHaveTextContent('Save & sign in');
    fireEvent.click(screen.getByTestId('mcp-form-save'));

    await waitFor(() => expect(onSaved).toHaveBeenCalledWith('notion', { signIn: true }));
    expect(mockConfigSet.mock.calls[0][0].mcpServers.notion).toEqual({
      url: 'https://mcp.notion.com/mcp',
    });
  });

  it('edits in place: stored names shown blank keep their value, a removed one is cleared', async () => {
    const onSaved = vi.fn();
    render(
      <McpServerForm
        existing={{
          server_id: 's1',
          qualified_name: 'github',
          display_name: 'github',
          command_kind: 'node',
          command: 'npx',
          args: ['-y', 'x'],
          env_keys: ['GITHUB_TOKEN', 'OTHER', '__oauth__'],
          installed_at: 1,
          transport: { kind: 'stdio' },
          enabled: false,
        }}
        onClose={() => {}}
        onSaved={onSaved}
      />
    );
    expect(screen.getByTestId('mcp-form-name')).toHaveValue('github');
    expect(screen.getByTestId('mcp-form-args')).toHaveValue('-y x');
    // Two stored names, none of the bookkeeping ones, and no values.
    const keys = screen.getAllByLabelText('Variable name') as HTMLInputElement[];
    expect(keys.map(k => k.value)).toEqual(['GITHUB_TOKEN', 'OTHER']);
    expect(screen.getAllByPlaceholderText('Stored — leave blank to keep')).toHaveLength(2);

    // Drop OTHER, keep GITHUB_TOKEN untouched.
    fireEvent.click(screen.getAllByRole('button', { name: 'Remove row' })[1]);
    fireEvent.click(screen.getByTestId('mcp-form-save'));

    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(mockConfigSet.mock.calls[0][0].mcpServers.github).toEqual({
      command: 'npx',
      args: ['-y', 'x'],
      env: { OTHER: '' },
      enabled: false,
    });
  });

  it("shows the core's refusal and stays open", async () => {
    mockConfigSet.mockRejectedValue(new Error('`bad` has a `cwd` field'));
    render(<McpServerForm onClose={() => {}} onSaved={() => {}} />);
    fireEvent.change(screen.getByTestId('mcp-form-name'), { target: { value: 'bad' } });
    fireEvent.change(screen.getByTestId('mcp-form-command'), { target: { value: 'x' } });
    fireEvent.click(screen.getByTestId('mcp-form-save'));
    expect(await screen.findByTestId('mcp-form-error')).toHaveTextContent('`cwd`');
    expect(screen.getByTestId('mcp-server-form')).toBeInTheDocument();
  });
});
