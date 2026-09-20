import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import McpJsonEditor from './McpJsonEditor';

const mockConfigGet = vi.fn();
const mockConfigSet = vi.fn();

vi.mock('../../../services/api/mcpClientsApi', () => ({
  mcpClientsApi: {
    configGet: (...args: unknown[]) => mockConfigGet(...args),
    configSet: (...args: unknown[]) => mockConfigSet(...args),
  },
}));

const LOADED = {
  mcpServers: {
    echo: { command: 'npx', args: ['-y', 'echo'], envKeys: ['TOKEN'], authConfigured: true },
  },
};

const textarea = () => screen.getByTestId('mcp-json-textarea') as HTMLTextAreaElement;

describe('McpJsonEditor', () => {
  beforeEach(() => {
    mockConfigGet.mockReset();
    mockConfigSet.mockReset();
    mockConfigGet.mockResolvedValue(LOADED);
  });

  it('loads the document and shows it formatted, with Save and Revert idle', async () => {
    render(<McpJsonEditor />);
    expect(screen.getByTestId('mcp-json-loading')).toBeInTheDocument();
    await screen.findByTestId('mcp-json-editor');
    expect(textarea().value).toContain('"mcpServers"');
    expect(textarea().value).toContain('"echo"');
    // A read never shows a value; it shows the names and the flag.
    expect(textarea().value).toContain('"envKeys"');
    expect(textarea().value).toContain('"authConfigured": true');
    expect(screen.getByTestId('mcp-json-save')).toBeDisabled();
    expect(screen.getByTestId('mcp-json-revert')).toBeDisabled();
  });

  it('refuses to render an empty editor when the read fails', async () => {
    mockConfigGet.mockRejectedValue(new Error('core down'));
    render(<McpJsonEditor />);
    await screen.findByTestId('mcp-json-error');
    expect(screen.queryByTestId('mcp-json-textarea')).not.toBeInTheDocument();
    // Retry re-reads.
    mockConfigGet.mockResolvedValue(LOADED);
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    await screen.findByTestId('mcp-json-editor');
  });

  it('shows a local parse error and keeps Save disabled for a broken buffer', async () => {
    render(<McpJsonEditor />);
    await screen.findByTestId('mcp-json-editor');
    fireEvent.change(textarea(), { target: { value: '{ "mcpServers": { "notion": {} } }' } });
    expect(screen.getByTestId('mcp-json-parse-error').textContent).toContain('`notion`');
    expect(screen.getByTestId('mcp-json-save')).toBeDisabled();
    // Revert restores the loaded text.
    fireEvent.click(screen.getByTestId('mcp-json-revert'));
    expect(textarea().value).toContain('"echo"');
    expect(screen.queryByTestId('mcp-json-parse-error')).not.toBeInTheDocument();
  });

  it('saves a valid edit, reloads the rendered result and reports what changed', async () => {
    const onSaved = vi.fn();
    mockConfigSet.mockResolvedValue({
      mcpServers: { hosted: { url: 'https://h.test/mcp', authConfigured: true } },
      added: ['hosted'],
      updated: [],
      removed: ['echo'],
    });
    render(<McpJsonEditor onSaved={onSaved} />);
    await screen.findByTestId('mcp-json-editor');

    fireEvent.change(textarea(), {
      target: {
        value: JSON.stringify({
          mcpServers: { hosted: { url: 'https://h.test/mcp', headers: { Authorization: 'x' } } },
        }),
      },
    });
    expect(screen.getByTestId('mcp-json-save')).toBeEnabled();
    fireEvent.click(screen.getByTestId('mcp-json-save'));

    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(mockConfigSet).toHaveBeenCalledWith({
      mcpServers: { hosted: { url: 'https://h.test/mcp', headers: { Authorization: 'x' } } },
    });
    // The buffer is what the core rendered back: no credential in it.
    expect(textarea().value).toContain('"hosted"');
    expect(textarea().value).not.toContain('Authorization');
    expect(screen.getByTestId('mcp-json-saved').textContent).toContain('1 added');
    expect(screen.getByTestId('mcp-json-saved').textContent).toContain('1 removed');
  });

  it("shows the core's refusal verbatim and keeps the user's text", async () => {
    mockConfigSet.mockRejectedValue(
      new Error('`bad` has a `cwd` field this host doesn\'t understand')
    );
    render(<McpJsonEditor />);
    await screen.findByTestId('mcp-json-editor');
    const text = JSON.stringify({ mcpServers: { bad: { command: 'x', cwd: '/tmp' } } });
    fireEvent.change(textarea(), { target: { value: text } });
    fireEvent.click(screen.getByTestId('mcp-json-save'));

    await screen.findByTestId('mcp-json-refusal');
    expect(screen.getByTestId('mcp-json-refusal').textContent).toContain('`cwd`');
    expect(textarea().value).toBe(text);
  });
});
