import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { ToolTimelineEntry } from '../../../store/chatRuntimeSlice';
import { ParallelAgentsCard } from './ParallelAgentsCard';

vi.mock('../../../lib/i18n/I18nContext', () => ({ useT: () => ({ t: (key: string) => key }) }));
vi.mock('../../../providers/AssistantUiRuntimeProvider', () => ({ useAuiThreadId: () => 't1' }));
vi.mock('./SubagentActivityCard', () => ({ SubagentActivityCard: () => null }));

const timeline = vi.hoisted(() => ({ entries: [] as ToolTimelineEntry[] }));
vi.mock('../../../store/hooks', () => ({
  useAppSelector: (select: (state: unknown) => unknown) =>
    select({ chatRuntime: { toolTimelineByThread: { t1: timeline.entries } } }),
}));

function child(id: string, seq: number, status: string): ToolTimelineEntry {
  return {
    id,
    name: 'subagent',
    round: 1,
    seq,
    status: status === 'running' ? 'running' : 'success',
    subagent: { taskId: id, agentId: id, status, parentCallId: 'call-p', toolCalls: [] },
  } as ToolTimelineEntry;
}

function renderCard(status: 'running' | 'complete') {
  return render(
    <ParallelAgentsCard
      type="tool-call"
      toolName="spawn_parallel_agents"
      toolCallId="call-p"
      args={{} as never}
      argsText="{}"
      result={undefined}
      status={{ type: status } as never}
      addResult={() => {}}
      resume={() => {}}
      respondToApproval={async () => {}}
    />
  );
}

describe('ParallelAgentsCard', () => {
  it('counts complete, running and failed workers', () => {
    timeline.entries = [
      child('a', 1, 'running'),
      child('b', 2, 'completed'),
      child('c', 3, 'failed'),
    ];
    renderCard('running');
    const header = screen.getByTestId('parallel-agents-header');
    expect(header).toHaveTextContent('chat.subagents.ofTotal');
    expect(header).toHaveTextContent('chat.subagents.runningCount · chat.subagents.failedCount');
    expect(screen.queryByTestId('parallel-agents-aggregating')).toBeNull();
  });

  it('reports an incomplete worker as partial rather than silently complete', () => {
    timeline.entries = [child('a', 1, 'completed'), child('b', 2, 'incomplete')];
    renderCard('complete');
    const header = screen.getByTestId('parallel-agents-header');
    expect(header).toHaveTextContent('chat.subagents.incompleteCount');
    expect(header).not.toHaveTextContent('chat.subagents.failedCount');
  });

  it('says the parent is processing once every worker is back but the call is still open', () => {
    timeline.entries = [child('a', 1, 'completed'), child('b', 2, 'completed')];
    renderCard('running');
    expect(screen.getByTestId('parallel-agents-aggregating')).toHaveTextContent(
      'chat.subagents.settled'
    );
  });

  it('drops the processing line once the call settles', () => {
    timeline.entries = [child('a', 1, 'completed')];
    renderCard('complete');
    expect(screen.queryByTestId('parallel-agents-aggregating')).toBeNull();
  });
});
