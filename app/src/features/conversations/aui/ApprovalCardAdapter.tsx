'use client';

/**
 * OpenHuman glue over the vendored `elements/approval-card.tsx`.
 *
 * Two call sites share this one adapter (per the assistant-ui-elements plan,
 * WS-B row: "Build ONE adapter component for this out-of-thread case"):
 *
 * - In-thread: `ChatToolParts.tsx`'s `GatedToolCall`, for a `Prompt`-class
 *   tool call parked on the ApprovalGate and attached to its own tool-call
 *   part (`approval_request` socket event). Replaces the deleted
 *   `ApprovalRequestCard`.
 * - Out-of-thread: the composer-header decks in `Conversations.tsx` (a
 *   paused `tinyflows` run's `flow_approval_request`, or a background park
 *   with no owning thread/run — `approval_list_pending`), and the flow-run
 *   inspector's `FlowRunPendingApprovalCard`. Replaces the deleted
 *   `FlowApprovalRequestCard` / `UnroutedApprovalCard` / `ApprovalDecisionCard`.
 *
 * Every decision still routes through the single shared
 * `openhuman.approval_decide` RPC (`services/api/approvalApi.ts`'s
 * `decideApproval`) — this component owns only the deciding/error UI state
 * and the vendored element's props, never the RPC itself; callers pass
 * `onDecide`.
 */
import { useState } from 'react';
import debug from 'debug';

import { ApprovalCard } from '../../../components/assistant-ui/elements/approval-card';
import { useT } from '../../../lib/i18n/I18nContext';
import type { ApprovalDecision } from '../../../services/api/approvalApi';
import { formatCountdown, useApprovalExpirySeconds } from './approvalCountdown';

const log = debug('openhuman:aui:approval-card-adapter');

export interface ApprovalCardAdapterProps {
  ariaLabel: string;
  title: string;
  subtitle: string;
  /** The exact command/target rendered in the card's mono panel. */
  command: string;
  toolName: string;
  /** RFC3339 timestamp, or `null`/absent when the request does not expire. */
  expiresAt?: string | null;
  /**
   * Decision to send for "Always allow". Omit to hide that button entirely
   * (e.g. the unrouted-approval surface, which deliberately offers only
   * once/deny — see the deleted `UnroutedApprovalCard`'s doc comment).
   */
  alwaysDecision?: ApprovalDecision;
  alwaysHint?: string;
  onDecide: (decision: ApprovalDecision) => Promise<void>;
  /** Prefix for each button's `data-analytics-id` / e2e `data-testid`. */
  analyticsPrefix: string;
  testId?: string;
  className?: string;
  /** External busy flag (the unrouted deck shares one busy state across rows). */
  busy?: boolean;
}

export function ApprovalCardAdapter({
  ariaLabel,
  title,
  subtitle,
  command,
  toolName,
  expiresAt,
  alwaysDecision,
  alwaysHint,
  onDecide,
  analyticsPrefix,
  testId,
  className,
  busy = false,
}: ApprovalCardAdapterProps) {
  const { t } = useT();
  const [deciding, setDeciding] = useState<ApprovalDecision | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const expirySeconds = useApprovalExpirySeconds(expiresAt);

  const decide = async (decision: ApprovalDecision) => {
    if (deciding || busy) return;
    setDeciding(decision);
    setErrorMsg(null);
    try {
      await onDecide(decision);
    } catch (e) {
      log('decide(%s) failed: %o', decision, e);
      setErrorMsg(t('chat.approval.error'));
      setDeciding(null);
    }
  };

  const disabled = deciding !== null || busy;

  return (
    <div role="alertdialog" aria-label={ariaLabel} data-testid={testId} className={className}>
      <ApprovalCard
        state={deciding ? 'running' : 'request'}
        title={title}
        subtitle={subtitle}
        command={command || toolName}
        expiry={
          expirySeconds !== null ? (
            <span className="text-foreground/35 mt-0.5 text-[11px] tabular-nums">
              {t('chat.approval.expiresIn').replace('{time}', formatCountdown(expirySeconds))}
            </span>
          ) : undefined
        }
        denyLabel={t('chat.approval.deny')}
        alwaysAllowLabel={t('chat.approval.alwaysAllow')}
        allowOnceLabel={t('chat.approval.approve')}
        runningLabel={t('chat.approval.deciding')}
        onDeny={() => void decide('deny')}
        onAlwaysAllow={alwaysDecision ? () => void decide(alwaysDecision) : undefined}
        onAllowOnce={() => void decide('approve_once')}
        denyProps={{ 'data-analytics-id': `${analyticsPrefix}-deny`, disabled }}
        alwaysAllowProps={
          alwaysDecision
            ? {
                'data-analytics-id': `${analyticsPrefix}-always`,
                disabled,
                title: alwaysHint,
              }
            : undefined
        }
        allowOnceProps={{ 'data-analytics-id': `${analyticsPrefix}-approve-once`, disabled }}
      />
      {errorMsg && (
        <p role="alert" className="mt-2 text-xs text-coral-600 dark:text-coral-400">
          ⚠ {errorMsg}
        </p>
      )}
    </div>
  );
}
