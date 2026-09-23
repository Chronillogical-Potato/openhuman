/**
 * The approvals nobody else will show.
 *
 * A background trigger's park has no chat thread and no flow context, so the
 * gate publishes `ApprovalRequested` with `thread_id: None`, the web-channel
 * subscriber drops it ("NOT surfacing"), and it TTL-denies after 600 s with
 * the user never asked (openhuman#6406). This hook is the surface that asks.
 *
 * The assertions that matter are about the DISCRIMINATOR: it must show the
 * unclaimed rows and stay out of the way of the two surfaces that already
 * work. A hook that showed everything would double every chat approval; one
 * that showed nothing would be the bug it is fixing.
 */
import { configureStore } from '@reduxjs/toolkit';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  decideApproval,
  fetchPendingApprovals,
  type PendingApproval,
} from '../../services/api/approvalApi';
import chatRuntimeReducer, { setPendingApprovalForThread } from '../../store/chatRuntimeSlice';
import threadReducer from '../../store/threadSlice';
import { resetFlowPendingApprovalsStoreForTests } from '../flowPendingApprovalsStore';
import { useUnroutedApprovals } from '../useUnroutedApprovals';

vi.mock('../../services/api/approvalApi', async importOriginal => {
  const actual = await importOriginal<typeof import('../../services/api/approvalApi')>();
  return { ...actual, fetchPendingApprovals: vi.fn(), decideApproval: vi.fn() };
});

function row(over: Partial<PendingApproval> = {}): PendingApproval {
  return {
    request_id: 'req-bg',
    tool_name: 'triage.escalate',
    action_summary: 'triage::ESCALATE target=orchestrator',
    args_redacted: {},
    session_id: 's1',
    created_at: '2026-09-23T04:12:00.000Z',
    expires_at: '2026-09-23T04:22:00.000Z',
    ...over,
  };
}

function mount(seed?: (store: ReturnType<typeof buildStore>) => void) {
  const store = buildStore();
  seed?.(store);
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store}>{children}</Provider>
  );
  return renderHook(() => useUnroutedApprovals(), { wrapper });
}

function buildStore() {
  return configureStore({ reducer: { thread: threadReducer, chatRuntime: chatRuntimeReducer } });
}

beforeEach(() => {
  resetFlowPendingApprovalsStoreForTests();
  vi.mocked(decideApproval).mockResolvedValue(undefined as never);
});

afterEach(() => {
  resetFlowPendingApprovalsStoreForTests();
  vi.clearAllMocks();
});

describe('useUnroutedApprovals', () => {
  it('surfaces a park that has no thread and no flow', async () => {
    vi.mocked(fetchPendingApprovals).mockResolvedValue([row()]);
    const { result } = mount();

    await waitFor(() => expect(result.current.approvals).toHaveLength(1));
    expect(result.current.approvals[0]?.request_id).toBe('req-bg');
  });

  it('leaves a flow-scoped park to the flow surface', async () => {
    vi.mocked(fetchPendingApprovals).mockResolvedValue([
      row({
        request_id: 'req-flow',
        source_context: { kind: 'flow', flow_id: 'f1', run_id: 'r1' },
      }),
    ]);
    const { result } = mount();

    // Give the poll a chance to land before asserting an absence.
    await waitFor(() => expect(vi.mocked(fetchPendingApprovals)).toHaveBeenCalled());
    await waitFor(() => expect(result.current.approvals).toEqual([]));
  });

  it('leaves a chat-routed park to the transcript card', async () => {
    // Same row, twice: once as the durable pending row, once as the chat
    // surface's live copy. Showing it here as well would put one approval in
    // two places.
    vi.mocked(fetchPendingApprovals).mockResolvedValue([row({ request_id: 'req-chat' })]);
    const { result } = mount(store => {
      store.dispatch(
        setPendingApprovalForThread({
          threadId: 't1',
          approval: {
            requestId: 'req-chat',
            toolName: 'triage.escalate',
            message: 'in the transcript already',
          },
        })
      );
    });

    await waitFor(() => expect(vi.mocked(fetchPendingApprovals)).toHaveBeenCalled());
    await waitFor(() => expect(result.current.approvals).toEqual([]));
  });

  it('shows a background park raised alongside an unrelated chat approval', async () => {
    // The discriminator must exclude by request_id, not "is any chat approval
    // open" — otherwise one chat gate hides every background one behind it.
    vi.mocked(fetchPendingApprovals).mockResolvedValue([
      row({ request_id: 'req-chat' }),
      row({ request_id: 'req-bg' }),
    ]);
    const { result } = mount(store => {
      store.dispatch(
        setPendingApprovalForThread({
          threadId: 't1',
          approval: { requestId: 'req-chat', toolName: 'shell', message: 'in the transcript' },
        })
      );
    });

    await waitFor(() => expect(result.current.approvals).toHaveLength(1));
    expect(result.current.approvals[0]?.request_id).toBe('req-bg');
  });

  it('records a decision through the shared approval RPC', async () => {
    vi.mocked(fetchPendingApprovals).mockResolvedValue([row()]);
    const { result } = mount();
    await waitFor(() => expect(result.current.approvals).toHaveLength(1));

    await result.current.decide('req-bg', 'approve_once');

    expect(vi.mocked(decideApproval)).toHaveBeenCalledWith('req-bg', 'approve_once');
  });

  it('keeps the card and reports why when the decide fails', async () => {
    // A failed decide leaves the core still parked, so the prompt must stay
    // answerable rather than vanishing optimistically.
    vi.mocked(fetchPendingApprovals).mockResolvedValue([row()]);
    vi.mocked(decideApproval).mockRejectedValue(new Error('core unreachable'));
    const { result } = mount();
    await waitFor(() => expect(result.current.approvals).toHaveLength(1));

    await expect(result.current.decide('req-bg', 'approve_once')).rejects.toThrow(
      'core unreachable'
    );

    await waitFor(() => expect(result.current.error).toBe('core unreachable'));
    expect(result.current.approvals).toHaveLength(1);
  });
});
