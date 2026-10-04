import { fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { MemoryPolicy } from '../../services/api/memoryApi';
import { renderWithProviders } from '../../test/test-utils';
import MemoryConversationsTab from './MemoryConversationsTab';

const hoisted = vi.hoisted(() => ({
  policyGet: vi.fn(),
  policySet: vi.fn(),
  agents: vi.fn(),
  items: vi.fn(),
}));

vi.mock('../../services/api/memoryApi', async importOriginal => ({
  ...(await importOriginal<typeof import('../../services/api/memoryApi')>()),
  memoryPolicyGet: (...a: unknown[]) => hoisted.policyGet(...a),
  memoryPolicySet: (...a: unknown[]) => hoisted.policySet(...a),
  memoryAgentsList: (...a: unknown[]) => hoisted.agents(...a),
  memoryItemsList: (...a: unknown[]) => hoisted.items(...a),
}));

// The backfill card has its own suite.
vi.mock('./MemoryConversationsBackfill', () => ({
  default: () => <div data-testid="stub-backfill" />,
}));

const POLICY: MemoryPolicy = {
  log_conversations: true,
  recall: {
    enabled: true,
    budget_tokens: 1500,
    learnings_limit: 8,
    brain_limit: 6,
    history_limit: 6,
    team_limit: 4,
    build_beliefs_every: 20,
    pre_turn_timeout_ms: 1500,
    compaction_timeout_ms: 5000,
    build_delay_secs: 30,
  },
  root: 'user:me',
  agent_id: 'main',
  host_bound: false,
};

beforeEach(() => {
  hoisted.policyGet.mockReset().mockResolvedValue(POLICY);
  hoisted.policySet.mockReset();
  hoisted.agents.mockReset().mockResolvedValue({
    root: 'user:me',
    agents: [
      { agent_id: 'main', turns: 6 },
      { agent_id: 'researcher', turns: 2 },
    ],
  });
  hoisted.items.mockReset();
});

describe('MemoryConversationsTab', () => {
  it('shows the logging switch, the agents and the backfill card', async () => {
    renderWithProviders(<MemoryConversationsTab />);
    expect(await screen.findByTestId('memory-agent-main')).toHaveTextContent('6 turns');
    expect(screen.getByTestId('memory-agent-researcher')).toHaveTextContent('2 turns');
    expect(screen.getByTestId('memory-conversations-log')).toBeChecked();
    expect(screen.getByTestId('stub-backfill')).toBeInTheDocument();
  });

  it('turns turn logging off through the policy', async () => {
    hoisted.policySet.mockResolvedValue({ ...POLICY, log_conversations: false });
    renderWithProviders(<MemoryConversationsTab />);
    fireEvent.click(await screen.findByTestId('memory-conversations-log'));
    await waitFor(() =>
      expect(hoisted.policySet).toHaveBeenCalledWith({ log_conversations: false })
    );
    await waitFor(() => expect(screen.getByTestId('memory-conversations-log')).not.toBeChecked());
  });

  it("opens an agent's stored conversations and pages through them", async () => {
    hoisted.items
      .mockResolvedValueOnce({
        items: [{ id: 'c1', kind: 'conversation', text: 'We talked about Atlas', meta: {} }],
        next_cursor: 'n1',
      })
      .mockResolvedValueOnce({
        items: [{ id: 'c2', kind: 'conversation', text: 'And the launch', meta: {} }],
        next_cursor: null,
      });
    renderWithProviders(<MemoryConversationsTab />);
    fireEvent.click(await screen.findByTestId('memory-agent-researcher-open'));

    await waitFor(() =>
      expect(hoisted.items).toHaveBeenCalledWith({
        filter: { kinds: ['conversation'], agent_id: 'researcher' },
        limit: 20,
        cursor: undefined,
      })
    );
    expect(await screen.findByTestId('memory-hit-c1')).toHaveTextContent('We talked about Atlas');

    fireEvent.click(screen.getByTestId('memory-conversations-more'));
    expect(await screen.findByTestId('memory-hit-c2')).toBeInTheDocument();
    expect(screen.getByTestId('memory-hit-c1')).toBeInTheDocument();
    expect(hoisted.items).toHaveBeenLastCalledWith(expect.objectContaining({ cursor: 'n1' }));

    // Clicking again closes it.
    fireEvent.click(screen.getByTestId('memory-agent-researcher-open'));
    expect(screen.queryByTestId('memory-agent-researcher-items')).not.toBeInTheDocument();
  });

  it('says when an agent has nothing stored', async () => {
    hoisted.items.mockResolvedValue({ items: [], next_cursor: null });
    renderWithProviders(<MemoryConversationsTab />);
    fireEvent.click(await screen.findByTestId('memory-agent-main-open'));
    expect(await screen.findByTestId('memory-agent-main-items')).toHaveTextContent(
      'No conversations stored for this agent yet.'
    );
  });

  it('shows the empty agent list', async () => {
    hoisted.agents.mockResolvedValue({ root: 'user:me', agents: [] });
    renderWithProviders(<MemoryConversationsTab />);
    expect(await screen.findByTestId('memory-conversations-empty')).toBeInTheDocument();
  });

  it('shows a load error', async () => {
    hoisted.policyGet.mockRejectedValue(new Error('MEMORY_OFF'));
    hoisted.agents.mockRejectedValue(new Error('MEMORY_OFF'));
    renderWithProviders(<MemoryConversationsTab />);
    expect(await screen.findByTestId('memory-conversations-error')).toHaveTextContent('MEMORY_OFF');
    expect(screen.queryByTestId('memory-conversations-log')).not.toBeInTheDocument();
  });
});
