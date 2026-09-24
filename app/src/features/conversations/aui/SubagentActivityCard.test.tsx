import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { SubagentActivityCard } from './SubagentActivityCard';

function openDisclosure() {
  fireEvent.click(within(screen.getByTestId('assistant-ui-subagent-call')).getByRole('button'));
}

describe('SubagentActivityCard', () => {
  it('derives a working state from a running activity with no explicit status text needed', () => {
    render(
      <SubagentActivityCard
        activity={{ taskId: 't', agentId: 'researcher', status: 'running', toolCalls: [] }}
      />
    );
    expect(screen.getByTestId('assistant-ui-subagent-call')).toHaveAttribute(
      'data-status',
      'working'
    );
  });

  it('marks a failed delegation as failed rather than complete', () => {
    render(
      <SubagentActivityCard
        activity={{ taskId: 't', agentId: 'researcher', status: 'failed', toolCalls: [] }}
      />
    );
    expect(screen.getByTestId('assistant-ui-subagent-call')).toHaveAttribute(
      'data-status',
      'failed'
    );
  });

  it('marks a cancelled delegation as cancelled', () => {
    render(
      <SubagentActivityCard
        activity={{ taskId: 't', agentId: 'researcher', status: 'cancelled', toolCalls: [] }}
      />
    );
    expect(screen.getByTestId('assistant-ui-subagent-call')).toHaveAttribute(
      'data-status',
      'cancelled'
    );
  });

  it('shows the awaiting-user question as plain text with no reply box', () => {
    // This surface (the inline rail, the Agent Process Source panel) renders
    // outside an AssistantRuntimeProvider, so unlike `SubagentTaskCard` there
    // is nowhere to send a reply through — the question is read-only here.
    render(
      <SubagentActivityCard
        activity={{
          taskId: 't',
          agentId: 'researcher',
          status: 'awaiting_user',
          awaitingQuestion: 'Which repo should I use?',
          toolCalls: [],
        }}
      />
    );
    expect(screen.getByTestId('assistant-ui-subagent-call')).toHaveAttribute(
      'data-status',
      'awaiting_user'
    );
    expect(screen.getByTestId('subagent-awaiting-question')).toHaveTextContent(
      'Which repo should I use?'
    );
    expect(screen.queryByTestId('subagent-answer-input')).toBeNull();
  });

  it('renders the delegation label with the agent display name', () => {
    render(
      <SubagentActivityCard
        activity={{
          taskId: 't',
          agentId: 'researcher',
          displayName: 'Researcher',
          status: 'success',
          toolCalls: [],
        }}
      />
    );
    expect(screen.getByText('Delegated to Researcher')).toBeInTheDocument();
  });

  it('renders child tool calls (from `toolCalls`) inside the nested transcript once opened', () => {
    render(
      <SubagentActivityCard
        activity={{
          taskId: 't',
          agentId: 'researcher',
          status: 'success',
          toolCalls: [{ callId: 'c1', toolName: 'web_search', status: 'success' }],
        }}
      />
    );
    openDisclosure();
    expect(screen.getByTestId('subagent-activity')).toBeInTheDocument();
  });

  it('renders no nested-transcript disclosure chevron when there is nothing to show', () => {
    render(
      <SubagentActivityCard activity={{ taskId: 't', agentId: 'researcher', toolCalls: [] }} />
    );
    expect(screen.getByRole('button', { name: /Delegated to researcher/i })).toHaveAttribute(
      'disabled'
    );
  });
});
