/**
 * UnroutedApprovalCard — the banner for an approval no other surface claims.
 *
 * A park raised by a background trigger has no chat thread and no flow run, so
 * neither `ApprovalRequestCard` nor `FlowApprovalRequestCard` will ever show
 * it (openhuman#6406, general form openhuman#5746). {@link useUnroutedApprovals}
 * finds those rows in the durable `approval_list_pending` queue; this renders
 * one.
 *
 * Chrome is `ApprovalDecisionCard`, the same component the other two approval
 * surfaces use, so all three read as one affordance family rather than three
 * visually unrelated prompts for the same kind of decision.
 *
 * Only two decisions are offered. `approve_always_for_tool` is deliberately
 * absent: this prompt exists precisely because the request arrived from
 * attacker-influenceable content with no interactive session behind it, and a
 * session-wide standing allowlist is the wrong thing to grant from a banner
 * the user did not go looking for. Once is once.
 */
import debug from 'debug';
import { type FC, useState } from 'react';

import type { ApprovalDecision, PendingApproval } from '../../services/api/approvalApi';
import ApprovalDecisionCard, {
  type ApprovalDecisionAction,
} from '../approvals/ApprovalDecisionCard';

const log = debug('openhuman:chat:unrouted-approval-card');

const ACTION_DECISIONS: Record<string, ApprovalDecision> = {
  'unrouted-approval-approve': 'approve_once',
  'unrouted-approval-deny': 'deny',
};

const ACTIONS: ApprovalDecisionAction[] = [
  {
    id: 'unrouted-approval-approve',
    label: 'Approve once',
    busyLabel: 'Approving…',
    variant: 'primary',
  },
  {
    id: 'unrouted-approval-deny',
    label: 'Deny',
    busyLabel: 'Denying…',
    variant: 'secondary',
    tone: 'danger',
  },
];

/** `2026-09-23T04:12:00Z` -> `4:12 am`, or `''` when unparseable. */
function formatRaisedAt(iso: string): string {
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) return '';
  return at.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
}

interface Props {
  approval: PendingApproval;
  busy: boolean;
  onDecide: (requestId: string, decision: ApprovalDecision) => Promise<void>;
}

export const UnroutedApprovalCard: FC<Props> = ({ approval, busy, onDecide }) => {
  const [busyActionId, setBusyActionId] = useState<string | null>(null);

  const handleAction = (actionId: string) => {
    const decision = ACTION_DECISIONS[actionId];
    if (!decision || busy) return;
    setBusyActionId(actionId);
    log('deciding request_id=%s decision=%s', approval.request_id, decision);
    void onDecide(approval.request_id, decision)
      .catch(() => {
        // The hook owns the error message and renders it above this card; the
        // card only needs to stop looking busy so the decision can be retried.
      })
      .finally(() => setBusyActionId(null));
  };

  const raisedAt = formatRaisedAt(approval.created_at);

  return (
    <ApprovalDecisionCard
      ariaLabel={`Background approval required: ${approval.tool_name}`}
      testId="unrouted-approval-card"
      summary={
        <>
          <span className="font-medium">Background task needs approval</span>
          {' — '}
          {approval.action_summary || approval.tool_name}
        </>
      }
      metadata={
        <>
          <span data-testid="unrouted-approval-tool">{approval.tool_name}</span>
          {raisedAt && <span> · raised {raisedAt}</span>}
          {/* No thread to open: this ran with nobody watching, which is why it
              is here rather than in a transcript. */}
          <span> · no conversation</span>
        </>
      }
      actions={ACTIONS}
      busyActionId={busyActionId}
      onAction={handleAction}
    />
  );
};

export default UnroutedApprovalCard;
