import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { PendingApproval } from '../../services/api/approvalApi';
import { FlowRunPendingApprovalCard } from './FlowRunPendingApprovalCard';

const APPROVAL: PendingApproval = {
  request_id: 'request-1',
  tool_name: 'shell',
  action_summary: 'Run the release command',
  args_redacted: {},
  session_id: 'session-1',
  created_at: '2026-07-24T00:00:00Z',
  expires_at: null,
  source_context: { kind: 'flow', flow_id: 'flow-1', run_id: 'run-1' },
};

const TEST_ID_PREFIX = 'flow-run-pending-approval-request-1';

describe('FlowRunPendingApprovalCard', () => {
  it('renders run approval copy via the shared ApprovalCardAdapter', () => {
    render(<FlowRunPendingApprovalCard approval={APPROVAL} deciding={false} onDecide={vi.fn()} />);

    expect(screen.getByRole('alertdialog', { name: 'Pending approvals' })).toHaveAttribute(
      'data-testid',
      'flow-run-pending-approval-request-1'
    );
    expect(screen.getByText('Run the release command')).toBeInTheDocument();
    expect(screen.getByText('shell')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Allow once' })).toHaveAttribute(
      'data-analytics-id',
      `${TEST_ID_PREFIX}-approve-once`
    );
    expect(screen.getByRole('button', { name: 'Always allow' })).toHaveAttribute(
      'data-analytics-id',
      `${TEST_ID_PREFIX}-approve-always`
    );
    expect(screen.getByRole('button', { name: 'Deny' })).toHaveAttribute(
      'data-analytics-id',
      `${TEST_ID_PREFIX}-deny`
    );
  });

  it.each([
    ['Allow once', 'approve_once'],
    ['Always allow', 'approve_always_for_flow'],
    ['Deny', 'deny'],
  ] as const)('maps %s to %s', (label, decision) => {
    const onDecide = vi.fn().mockResolvedValue(undefined);
    render(<FlowRunPendingApprovalCard approval={APPROVAL} deciding={false} onDecide={onDecide} />);

    fireEvent.click(screen.getByRole('button', { name: label }));
    expect(onDecide).toHaveBeenCalledWith(decision);
  });

  it('disables every action while deciding', () => {
    const onDecide = vi.fn().mockReturnValue(new Promise<void>(() => undefined));
    render(<FlowRunPendingApprovalCard approval={APPROVAL} deciding={false} onDecide={onDecide} />);

    fireEvent.click(screen.getByRole('button', { name: 'Always allow' }));
    expect(screen.getByText('Approved, running')).toBeInTheDocument();
  });

  it('disables every action when already deciding on first render (external busy flag)', () => {
    render(<FlowRunPendingApprovalCard approval={APPROVAL} deciding onDecide={vi.fn()} />);

    expect(screen.getByRole('button', { name: 'Allow once' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Always allow' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Deny' })).toBeDisabled();
  });
});
