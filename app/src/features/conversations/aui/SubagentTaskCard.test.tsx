import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { SubagentActivity } from '../../../store/chatRuntimeSlice';
import { SubagentTaskCard } from './SubagentTaskCard';

const activity: SubagentActivity = {
  taskId: 'sub-1',
  agentId: 'researcher',
  displayName: 'Researcher',
  toolCalls: [],
  transcript: [{ kind: 'thinking', text: 'Checking primary sources.' }],
};

describe('SubagentTaskCard', () => {
  it('renders a running delegation with its nested transcript', () => {
    render(
      <SubagentTaskCard
        type="tool-call"
        toolName="task"
        toolCallId="sub-1"
        args={{ progress: activity } as never}
        argsText="{}"
        result={undefined}
        status={{ type: 'running' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={async () => {}}
      />
    );

    expect(screen.getByTestId('assistant-ui-subagent-call')).toHaveAttribute('data-state', 'working');
    expect(screen.getByText('Researcher')).toBeInTheDocument();
    expect(screen.getByTestId('subagent-activity')).toBeInTheDocument();
    expect(screen.getByText('Checking primary sources.')).toBeInTheDocument();
  });

  it('renders a failed delegation as failed, not as a completed one', () => {
    render(
      <SubagentTaskCard
        type="tool-call"
        toolName="task"
        toolCallId="sub-1"
        args={{} as never}
        argsText="{}"
        result={{ status: 'error', activity: { ...activity, status: 'failed' } } as never}
        status={{ type: 'complete', reason: 'stop' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={async () => {}}
      />
    );

    expect(screen.getByTestId('assistant-ui-subagent-call')).toHaveAttribute('data-status', 'failed');
  });

  it('renders the awaiting-user reply box and lets the answer through the composer path', async () => {
    render(
      <SubagentTaskCard
        type="tool-call"
        toolName="task"
        toolCallId="sub-1"
        args={
          {
            progress: { ...activity, status: 'awaiting_user', awaitingQuestion: 'Which repo?' },
          } as never
        }
        argsText="{}"
        result={undefined}
        status={{ type: 'requires-action', reason: 'interrupt' }}
        addResult={() => {}}
        resume={() => {}}
        respondToApproval={async () => {}}
      />
    );

    expect(screen.getByTestId('subagent-awaiting-user')).toBeInTheDocument();
    expect(screen.getByTestId('subagent-awaiting-question')).toHaveTextContent('Which repo?');
  });
});
