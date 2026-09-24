import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { ToolFallbackApproval } from '../tool-fallback';

vi.mock('@assistant-ui/react', async () => {
  const actual = await vi.importActual<typeof import('@assistant-ui/react')>('@assistant-ui/react');
  return { ...actual, useAuiState: () => false };
});

describe('ToolFallbackApproval — free-text answer path', () => {
  it('renders the plain decision bar when the request has no options/display', async () => {
    const respondToApproval = vi.fn().mockResolvedValue(undefined);
    render(
      <ToolFallbackApproval
        approval={{ id: 'a-1' }}
        respondToApproval={respondToApproval}
        status={{ type: 'requires-action', reason: 'interrupt' }}
      />
    );
    await userEvent.click(screen.getByText('Allow'));
    expect(respondToApproval).toHaveBeenCalledWith({ approved: true });
  });

  it('renders a Textarea + Send for a question (display: "text") and answers with the typed text', async () => {
    const respondToApproval = vi.fn().mockResolvedValue(undefined);
    render(
      <ToolFallbackApproval
        approval={{ id: 'a-2', display: 'text', allowFreeform: true, prompt: 'What is the title?' }}
        respondToApproval={respondToApproval}
        status={{ type: 'requires-action', reason: 'interrupt' }}
      />
    );
    expect(screen.getByText('What is the title?')).toBeInTheDocument();
    const textarea = screen.getByRole('textbox');
    await userEvent.type(textarea, 'Quarterly Deck');
    await userEvent.click(screen.getByText('Send'));
    expect(respondToApproval).toHaveBeenCalledWith({ text: 'Quarterly Deck' });
  });

  it('does not offer a bare Deny button for a question-mode request', () => {
    render(
      <ToolFallbackApproval
        approval={{ id: 'a-3', display: 'select', options: [{ id: 'o1', kind: 'allow-once' }] }}
        respondToApproval={vi.fn()}
        status={{ type: 'requires-action', reason: 'interrupt' }}
      />
    );
    expect(screen.queryByText('Deny')).toBeNull();
  });

  it('renders nothing once the approval is already resolved', () => {
    const { container } = render(
      <ToolFallbackApproval
        approval={{ id: 'a-4', approved: true }}
        respondToApproval={vi.fn()}
        status={{ type: 'requires-action', reason: 'interrupt' }}
      />
    );
    expect(container).toBeEmptyDOMElement();
  });
});
