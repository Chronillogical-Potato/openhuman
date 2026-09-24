import { configureStore } from '@reduxjs/toolkit';
import type React from 'react';
import { render, screen } from '@testing-library/react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';

import type { SubagentActivity, ToolTimelineEntry } from '../../../../store/chatRuntimeSlice';
import { ParallelAgentsCard } from '../ParallelAgentsCard';

const THREAD_ID = 'thread-1';
const PARENT_CALL_ID = 'call-spawn-parallel';

vi.mock('../../../../providers/AssistantUiRuntimeProvider', () => ({
  useAuiThreadId: () => THREAD_ID,
}));

function sub(partial: Partial<SubagentActivity> & { taskId: string }): SubagentActivity {
  return { agentId: 'researcher', toolCalls: [], parentCallId: PARENT_CALL_ID, ...partial };
}

function entry(id: string, status: ToolTimelineEntry['status'], subagent: SubagentActivity): ToolTimelineEntry {
  return { id, name: 'subagent:x', round: 0, seq: 0, status, subagent };
}

function buildStore(timeline: ToolTimelineEntry[]) {
  return configureStore({
    reducer: {
      chatRuntime: () => ({ toolTimelineByThread: { [THREAD_ID]: timeline } }),
    },
  });
}

function renderCard(timeline: ToolTimelineEntry[], toolCallId = PARENT_CALL_ID) {
  return render(
    <Provider store={buildStore(timeline)}>
      <ParallelAgentsCard
        {...({
          toolCallId,
          type: 'tool-call',
          toolName: 'spawn_parallel_agents',
          args: { tasks: [] },
          status: { type: 'running' },
          addResult: vi.fn(),
          resume: vi.fn(),
          respondToApproval: vi.fn(),
        } as unknown as React.ComponentProps<typeof ParallelAgentsCard>)}
      />
    </Provider>
  );
}

describe('ParallelAgentsCard', () => {
  it('renders nothing when no worker shares this call id', () => {
    const { container } = renderCard([
      entry('e1', 'running', sub({ taskId: 'sub-1', parentCallId: 'other-call' })),
    ]);
    expect(container.querySelector('[data-testid="assistant-ui-parallel-agents-call"]')).toBeNull();
  });

  it('renders the SubagentList + a TaskCard row per worker sharing parentCallId', () => {
    renderCard([
      entry('e1', 'running', sub({ taskId: 'sub-1', displayName: 'Researcher', status: 'running' })),
      entry('e2', 'success', sub({ taskId: 'sub-2', displayName: 'Archivist', status: 'completed' })),
      entry('e3', 'running', sub({ taskId: 'sub-3', parentCallId: 'other-call', displayName: 'Unrelated' })),
    ]);

    expect(screen.getAllByText('Researcher').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Archivist').length).toBeGreaterThan(0);
    expect(screen.queryByText('Unrelated')).toBeNull();

    const rows = screen.getAllByTestId('assistant-ui-subagent-call');
    expect(rows).toHaveLength(2);
  });

  it('shows the aggregating summary row while a worker is still active', () => {
    renderCard([entry('e1', 'running', sub({ taskId: 'sub-1', status: 'running' }))]);
    expect(screen.getByText('Aggregating results')).toBeInTheDocument();
  });
});
