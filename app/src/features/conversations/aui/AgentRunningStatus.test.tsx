import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { AssistantUiRuntimeProvider } from '../../../providers/AssistantUiRuntimeProvider';
import { AgentRunningStatus } from './AgentRunningStatus';

describe('AgentRunningStatus', () => {
  it('falls back to the thinking indicator when assistant-ui has no tasks', () => {
    render(
      <AssistantUiRuntimeProvider>
        <AgentRunningStatus />
      </AssistantUiRuntimeProvider>
    );

    expect(screen.getByTestId('agent-running-status-thinking')).toBeInTheDocument();
    expect(screen.queryByTestId('agent-running-status-tasks')).not.toBeInTheDocument();
  });
});
