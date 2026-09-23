import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { SubagentActivity } from '../../../store/chatRuntimeSlice';
import { ChatToolFallback, ChatToolGroup } from './ChatToolParts';

const activity: SubagentActivity = {
  taskId: 'sub-1',
  agentId: 'researcher',
  displayName: 'Researcher',
  toolCalls: [],
  transcript: [{ kind: 'thinking', text: 'Checking primary sources.' }],
};

describe('ChatToolParts', () => {
  it('renders a running delegation collapsed by default', async () => {
    render(
      <ChatToolFallback
        type="tool-call"
        toolName="task"
        toolCallId="sub-1"
        args={{ progress: activity } as never}
        argsText="{}"
        result={undefined}
        status={{ type: 'running' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={() => {}}
      />
    );

    expect(screen.getByText('running')).toBeInTheDocument();
    expect(screen.getByText('Researcher')).toBeInTheDocument();
    expect(screen.queryByText('Checking primary sources.')).not.toBeInTheDocument();
    expect(screen.getByTestId('assistant-ui-subagent-call')).toHaveAttribute(
      'data-state',
      'closed'
    );
    await userEvent.click(screen.getByRole('button', { name: /Delegated to Researcher/i }));
    expect(screen.getByText('Checking primary sources.')).toBeInTheDocument();
  });

  it('renders a failed delegation as failed, not as a completed one', () => {
    // `SubagentActivity.status` carries `failed`, but a settled part was read
    // as `running: false` and rendered with a success check — the transcript
    // reported a failure as a success.
    render(
      <ChatToolFallback
        type="tool-call"
        toolName="task"
        toolCallId="sub-1"
        args={{} as never}
        argsText="{}"
        result={{ ...activity, status: 'failed' } as never}
        status={{ type: 'complete' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={() => {}}
      />
    );

    expect(screen.getByTestId('assistant-ui-subagent-call')).toHaveAttribute(
      'data-status',
      'failed'
    );
    expect(screen.getByText('failed')).toBeInTheDocument();
    expect(screen.queryByText('running')).not.toBeInTheDocument();
  });

  it('keeps a completed delegation reading as completed', () => {
    render(
      <ChatToolFallback
        type="tool-call"
        toolName="task"
        toolCallId="sub-1"
        args={{} as never}
        argsText="{}"
        result={{ ...activity, status: 'completed' } as never}
        status={{ type: 'complete' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={() => {}}
      />
    );

    expect(screen.getByTestId('assistant-ui-subagent-call')).toHaveAttribute(
      'data-status',
      'completed'
    );
    expect(screen.queryByText('failed')).not.toBeInTheDocument();
  });

  it('keeps a still-running delegation running when the part has already settled', () => {
    // The tool-call status and the delegation status are separate fields, so a
    // settled part can still carry an in-flight activity. Hard-coding
    // `running: false` for any settled part froze that row into a success.
    render(
      <ChatToolFallback
        type="tool-call"
        toolName="task"
        toolCallId="sub-1"
        args={{} as never}
        argsText="{}"
        result={{ ...activity, status: 'running' } as never}
        status={{ type: 'complete' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={() => {}}
      />
    );

    expect(screen.getByText('running')).toBeInTheDocument();
  });

  it('does not show a success icon beside a cancelled tool', () => {
    // The adapter forwards `cancelled` now, and the card gated its non-success
    // icon on `error` alone — so the check icon sat next to the word
    // "cancelled". `failed` still gates the failure-explanation block, which
    // only an `error` carries.
    const { container } = render(
      <ChatToolFallback
        type="tool-call"
        toolName="web_search"
        toolCallId="call-1"
        args={{} as never}
        argsText="{}"
        result={{ status: 'cancelled' } as never}
        status={{ type: 'complete' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={() => {}}
      />
    );

    expect(screen.getByText('cancelled')).toBeInTheDocument();
    const card = screen.getByTestId('assistant-ui-tool-call');
    expect(card.querySelector('.lucide-circle-x')).not.toBeNull();
    expect(card.querySelector('.lucide-check')).toBeNull();
    expect(container).toBeTruthy();
  });

  it('opens a group containing in-flight work on mount', () => {
    render(
      <ChatToolGroup group={{ type: 'group-tool-call', status: { type: 'running' }, indices: [0] }}>
        <span>live delegation</span>
      </ChatToolGroup>
    );

    expect(screen.getByText('live delegation')).toBeVisible();
  });

  it('renders ordinary tools with rich input and output on the assistant-ui surface', async () => {
    render(
      <ChatToolFallback
        type="tool-call"
        toolName="web_search_tool"
        toolCallId="search-1"
        args={{ query: 'Lean open conjectures' } as never}
        argsText={'{"query":"Lean open conjectures"}'}
        result="Found 12 candidate problems"
        status={{ type: 'complete' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={() => {}}
      />
    );

    expect(screen.getByTestId('assistant-ui-tool-call')).toHaveTextContent(
      'Searching: Lean open conjectures'
    );
    await userEvent.click(screen.getByRole('button', { name: /Searching: Lean open conjectures/ }));
    expect(screen.getAllByText(/Lean open conjectures/).length).toBeGreaterThan(0);
    expect(screen.queryByText('Query', { exact: true })).not.toBeInTheDocument();
    expect(screen.getByText('Found 12 candidate problems')).toBeInTheDocument();
  });

  it('unwraps a single content field instead of showing a redundant title', async () => {
    render(
      <ChatToolFallback
        type="tool-call"
        toolName="web_fetch"
        toolCallId="fetch-1"
        args={{} as never}
        argsText="{}"
        result={{ content: '**Example Domain**', tool_call_id: 'fetch-1', success: true }}
        status={{ type: 'complete' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={() => {}}
      />
    );

    await userEvent.click(screen.getByRole('button', { name: /Fetching/ }));
    expect(screen.getByRole('strong')).toHaveTextContent('Example Domain');
    expect(screen.queryByText('Content', { exact: true })).not.toBeInTheDocument();
  });

  /**
   * The label comes from the tool's identity, never from a substring of its
   * name or from which argument keys it has. The old guess labelled all of
   * these "Searched the web": an email turn that found its Gmail action with
   * `tool_search` read as a web search from start to finish.
   */
  function renderTool(toolName: string, args: Record<string, unknown>, extra = {}) {
    render(
      <ChatToolFallback
        type="tool-call"
        toolName={toolName}
        toolCallId={`call-${toolName}`}
        args={args as never}
        argsText={JSON.stringify(args)}
        result="ok"
        status={{ type: 'complete' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={() => {}}
        {...extra}
      />
    );
    return screen.getByTestId('assistant-ui-tool-call');
  }

  it('labels tool discovery as a tool search, not a web search', () => {
    const card = renderTool('tool_search', { query: 'send an email with gmail' });
    expect(card).toHaveTextContent('Searching tools');
    expect(card).not.toHaveTextContent(/web/i);
  });

  it('labels a Composio action with a query argument by its service', () => {
    const card = renderTool('GMAIL_FETCH_EMAILS', { query: 'from:broker' });
    expect(card).toHaveTextContent('Making requests to your Gmail account');
    expect(card).not.toHaveTextContent(/web/i);
  });

  it('labels a tool_call bridge by the tool it invokes', () => {
    const card = renderTool('tool_call', {
      name: 'GMAIL_SEND_EMAIL',
      arguments: { recipient_email: 'me@example.com', subject: 'AAPL' },
    });
    expect(card).toHaveTextContent('Making requests to your Gmail account');
    expect(card).not.toHaveTextContent(/web/i);
  });

  it('labels memory searches as memory, not the web', () => {
    const card = renderTool('memory_hybrid_search', { query: 'apple stock' });
    expect(card).toHaveTextContent('Searching memory');
    expect(card).not.toHaveTextContent(/web/i);
  });

  it('prefers the label the row carries on the part artifact', () => {
    const card = renderTool(
      'stock_quote',
      { symbol: 'AAPL' },
      { artifact: { displayName: 'Stock quote', detail: 'AAPL' } }
    );
    expect(card).toHaveTextContent('Stock quote');
    expect(card).toHaveTextContent('AAPL');
  });

  it('humanises an unknown tool rather than guessing its category', () => {
    const card = renderTool('tool', { query: 'latest world news' });
    expect(card).not.toHaveTextContent(/web/i);
  });
});
