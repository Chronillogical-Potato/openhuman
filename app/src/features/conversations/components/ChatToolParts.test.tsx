import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { ChatToolFallback } from './ChatToolParts';

// The `task` toolkit entry (`aui/toolkit.tsx`) now renders `SubagentTaskCard`,
// not anything in this file — its own colocated test is
// `aui/SubagentTaskCard.test.tsx`. `ChatToolFallback` never special-cased
// `task` (the toolkit resolves it first regardless), so its tests below are
// unaffected by that move.
describe('ChatToolParts', () => {
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
        respondToApproval={async () => {}}
      />
    );

    expect(screen.getByText('cancelled')).toBeInTheDocument();
    const card = screen.getByTestId('assistant-ui-tool-call');
    expect(card.querySelector('.lucide-circle-x')).not.toBeNull();
    expect(card.querySelector('.lucide-check')).toBeNull();
    expect(container).toBeTruthy();
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
        respondToApproval={async () => {}}
      />
    );

    expect(screen.getByTestId('assistant-ui-tool-call')).toHaveTextContent('Searched the web');
    await userEvent.click(screen.getByRole('button', { name: /Searched the web/ }));
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
        respondToApproval={async () => {}}
      />
    );

    await userEvent.click(screen.getByRole('button', { name: /Read webpage/ }));
    expect(screen.getByRole('strong')).toHaveTextContent('Example Domain');
    expect(screen.queryByText('Content', { exact: true })).not.toBeInTheDocument();
  });

  // The old card called any call with a `query` argument "Searched the web",
  // which mislabelled memory, tool and email searches. A call whose name
  // degraded to `tool` is now labelled as what is known about it: an
  // unnamed tool, with its query as the chip.
  it('does not guess a web search from a query argument alone', () => {
    render(
      <ChatToolFallback
        type="tool-call"
        toolName="tool"
        toolCallId="search-generic"
        args={{ query: 'latest world news' } as never}
        argsText={'{"query":"latest world news"}'}
        result="# Search results\n\n- Headline"
        status={{ type: 'complete' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={async () => {}}
      />
    );

    const card = screen.getByTestId('assistant-ui-tool-call');
    expect(card).not.toHaveTextContent('Searched the web');
    expect(card).toHaveTextContent('Used tool');
    expect(card).toHaveTextContent('latest world news');
  });

  it('names the tool-discovery bridge for what it is, not a web search', () => {
    // `tool_search` ranks the DEFERRED TOOL CATALOGUE — Composio actions, MCP
    // tools — and never touches the network. It matched `looksLikeSearch`
    // twice (its name contains "search" AND its argument is `query`), so a
    // turn that fetched the user's own Google Calendar through Composio opened
    // with "Searched the web".
    render(
      <ChatToolFallback
        type="tool-call"
        toolName="tool_search"
        toolCallId="bridge-search"
        args={{ query: 'list calendar events google calendar' } as never}
        argsText={'{"query":"list calendar events google calendar"}'}
        result="1 match(es). Invoke one with tool_call"
        status={{ type: 'complete' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={async () => {}}
      />
    );

    expect(screen.getByTestId('assistant-ui-tool-call')).toHaveTextContent('Found tools');
    // The regression this pins: BOTH heuristic legs still match this row, so
    // dropping the explicit branch renders the web label again.
    expect(screen.getByTestId('assistant-ui-tool-call')).not.toHaveTextContent('Searched the web');
  });

  it('labels a Composio action with a query argument by its service, not the web', () => {
    // `GMAIL_FETCH_EMAILS` carries a `query` argument, which used to be the
    // web-search heuristic's whole trigger — any call with a `query` key read
    // as "Searched the web" regardless of what it actually called.
    render(
      <ChatToolFallback
        type="tool-call"
        toolName="GMAIL_FETCH_EMAILS"
        toolCallId="gmail-fetch"
        args={{ query: 'from:broker' } as never}
        argsText={'{"query":"from:broker"}'}
        result="1 message"
        status={{ type: 'complete' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={async () => {}}
      />
    );

    const card = screen.getByTestId('assistant-ui-tool-call');
    expect(card).toHaveTextContent('Used Gmail');
    expect(card).not.toHaveTextContent('Searched the web');
  });

  it('labels a memory search as memory, not the web', () => {
    render(
      <ChatToolFallback
        type="tool-call"
        toolName="memory_hybrid_search"
        toolCallId="memory-search"
        args={{ query: 'apple stock' } as never}
        argsText={'{"query":"apple stock"}'}
        result="2 memories"
        status={{ type: 'complete' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={async () => {}}
      />
    );

    const card = screen.getByTestId('assistant-ui-tool-call');
    expect(card).toHaveTextContent('Searched memory');
    expect(card).not.toHaveTextContent('Searched the web');
  });

  it('prefers the label the row carries on the part artifact for a tool the registry cannot describe', () => {
    render(
      <ChatToolFallback
        type="tool-call"
        toolName="acme_widget_ping"
        toolCallId="widget-ping"
        args={{ symbol: 'AAPL' } as never}
        argsText={'{"symbol":"AAPL"}'}
        result="ok"
        status={{ type: 'complete' }}
        artifact={{ kind: 'openhuman-tool', displayName: 'Widget ping', detail: 'AAPL' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={async () => {}}
      />
    );

    const card = screen.getByTestId('assistant-ui-tool-call');
    expect(card).toHaveTextContent('Widget ping');
    expect(card).toHaveTextContent('AAPL');
  });

  it('labels the tool_call wrapper by the tool it actually invoked', () => {
    // `tool_call_schema()` declares `{name, arguments}`, both required, so the
    // wrapped tool is always in `name`. The row used to show only the wrapper,
    // so the one call that fetched the user's calendar rendered "Tool Call".
    render(
      <ChatToolFallback
        type="tool-call"
        toolName="tool_call"
        toolCallId="bridge-call"
        args={{ name: 'GOOGLECALENDAR_EVENTS_LIST', arguments: { calendarId: 'primary' } } as never}
        argsText={'{"name":"GOOGLECALENDAR_EVENTS_LIST"}'}
        result="Items: Product Team Standup"
        status={{ type: 'complete' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={async () => {}}
      />
    );

    // Composio slugs carry no separator inside the toolkit name, so this also
    // pins the `googlecalendar` catalog lookup: without it the row degrades to
    // the raw "GOOGLECALENDAR EVENTS LIST".
    const card = screen.getByTestId('assistant-ui-tool-call');
    expect(card).toHaveTextContent('Used Google Calendar');
    expect(card).toHaveTextContent('Events list');
    expect(card).not.toHaveTextContent('GOOGLECALENDAR');
    expect(screen.getByTestId('assistant-ui-tool-call')).not.toHaveTextContent('Tool Call');
  });
});
