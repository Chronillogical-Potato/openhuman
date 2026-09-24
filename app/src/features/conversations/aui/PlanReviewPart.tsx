import type { ToolCallMessagePartComponent } from '@assistant-ui/react';
import debug from 'debug';
import { useCallback, useState } from 'react';

import { AgentPlan } from '../../../components/assistant-ui/elements/agent-plan';
import { ApprovalCard } from '../../../components/assistant-ui/elements/approval-card';
import { field } from '../../../components/assistant-ui/elements/surfaces';
import { useT } from '../../../lib/i18n/I18nContext';
import { useAuiThreadId } from '../../../providers/AssistantUiRuntimeProvider';
import { callCoreRpc } from '../../../services/coreRpcClient';
import { clearPendingPlanReviewForThread, type PendingPlanReview } from '../../../store/chatRuntimeSlice';
import { useAppDispatch, useAppSelector } from '../../../store/hooks';
import { useThreadTodos } from './useThreadTodos';

const log = debug('openhuman:chat:plan-review-part');

type Decision = 'approve' | 'reject' | 'revise';

/**
 * Does the thread's live todo list match this plan 1:1 — same length, every
 * step's text equal to the todo item at the same position? A simple
 * positional string-equality check, deliberately not fuzzy: an exact match
 * means the agent turned this exact plan into its todo list, so the count of
 * `done` items is a faithful `activeIndex`.
 */
function activeIndexFromTodos(
  steps: readonly string[],
  todos: ReturnType<typeof useThreadTodos>
): number | null {
  if (!todos || todos.length !== steps.length) return null;
  const matches = todos.every((item, i) => item.content === steps[i]);
  if (!matches) return false as unknown as null; // unreachable; see guard below
  return todos.filter(item => item.status === 'completed').length;
}

/**
 * The plan + decision surface for a review that is STILL pending (the
 * caller only mounts this while `pendingPlanReviewByThread[threadId]` holds
 * this exact review). Shared between the toolkit's `request_plan_review`
 * render ({@link PlanReviewPart}) and the pre-C2 composer-header fallback in
 * `Conversations.tsx` (no `tool_call_id` on the event yet, so there is no
 * tool-call part to attach the review to).
 *
 * Ports the `openhuman.plan_review_decide` RPC + optimistic clear from the
 * old `PlanReviewCard.tsx` verbatim; only the presentation changed (the
 * vendored `AgentPlan` + `ApprovalCard` elements instead of a bespoke card).
 */
export function PlanReviewCardCore({
  threadId,
  review,
}: {
  threadId: string;
  review: PendingPlanReview;
}) {
  const { t } = useT();
  const dispatch = useAppDispatch();
  const todos = useThreadTodos(threadId);
  const [revising, setRevising] = useState(false);
  const [feedback, setFeedback] = useState('');
  const [deciding, setDeciding] = useState<Decision | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const matched = activeIndexFromTodos(review.steps, todos);
  const activeIndex = matched && matched > 0 ? matched : matched === 0 ? 0 : 0;

  const decide = useCallback(
    async (decision: Decision, feedbackText?: string) => {
      if (deciding) return;
      setDeciding(decision);
      setErrorMsg(null);
      try {
        await callCoreRpc({
          method: 'openhuman.plan_review_decide',
          params: { request_id: review.requestId, decision, feedback: feedbackText },
        });
        dispatch(clearPendingPlanReviewForThread({ threadId }));
      } catch (e) {
        log('plan_review_decide failed: %o', e);
        setErrorMsg(t('chat.approval.error'));
        setDeciding(null);
      }
    },
    [deciding, dispatch, review.requestId, t, threadId]
  );

  const submitFeedback = useCallback(() => {
    const trimmed = feedback.trim();
    if (!trimmed) return;
    void decide('revise', trimmed);
  }, [decide, feedback]);

  return (
    <div className="flex w-full max-w-sm flex-col gap-3" data-testid="plan-review-card">
      <AgentPlan steps={review.steps} activeIndex={activeIndex} title={t('conversations.planReview.title')} />

      {errorMsg && <p className="text-xs text-red-600 dark:text-red-400">{errorMsg}</p>}

      <ApprovalCard
        state="request"
        command={review.summary || t('conversations.planReview.subtitle')}
        title={t('conversations.planReview.title')}
        subtitle={t('conversations.planReview.subtitle')}
        denyLabel={t('conversations.planReview.reject')}
        alwaysAllowLabel={t('conversations.planReview.revise')}
        allowOnceLabel={t('conversations.planReview.approve')}
        onDeny={deciding ? undefined : () => void decide('reject')}
        onAlwaysAllow={deciding ? undefined : () => setRevising(prev => !prev)}
        onAllowOnce={deciding ? undefined : () => void decide('approve')}
        denyProps={{ 'data-analytics-id': 'plan-review-reject' }}
        alwaysAllowProps={{ 'data-analytics-id': 'plan-review-send-feedback' }}
        allowOnceProps={{ 'data-analytics-id': 'plan-review-approve' }}
      />

      {revising && (
        <div>
          <label
            htmlFor={`plan-review-feedback-${review.requestId}`}
            className="mb-1 block text-xs font-medium text-foreground/60">
            {t('conversations.planReview.feedbackLabel')}
          </label>
          <textarea
            id={`plan-review-feedback-${review.requestId}`}
            data-testid="plan-review-feedback"
            value={feedback}
            onChange={e => setFeedback(e.target.value)}
            onKeyDown={e => {
              if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
                e.preventDefault();
                submitFeedback();
              }
            }}
            rows={2}
            disabled={deciding !== null}
            placeholder={t('conversations.planReview.feedbackPlaceholder')}
            className={`${field} w-full resize-y rounded-xl px-3 py-2 text-sm outline-none disabled:opacity-50`}
          />
          <div className="mt-1.5 flex justify-end">
            <button
              type="button"
              data-analytics-id="plan-review-send-feedback-submit"
              onClick={submitFeedback}
              disabled={deciding !== null || feedback.trim().length === 0}
              className="text-foreground/70 hover:bg-foreground/[0.06] hover:text-foreground/95 h-7 rounded-full px-2.5 text-xs font-medium transition-[background-color,color,scale] duration-150 active:scale-[0.96] disabled:pointer-events-none disabled:opacity-30">
              {deciding === 'revise' ? t('chat.approval.deciding') : t('conversations.planReview.sendFeedback')}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * Toolkit render for the `request_plan_review` tool call. Attaches to the
 * parked review by `tool_call_id` when the core sends one (lands with C2);
 * before that, falls back to "any pending review for this thread" — the
 * current core behavior only ever parks one review per thread at a time.
 *
 * A past/replayed call with no matching pending entry (already decided, or
 * history from a resumed session) renders the plan alone, fully "done"
 * visually (`activeIndex: steps.length`) — it is not this render's job to
 * re-offer a decision that was already made.
 */
export const PlanReviewPart: ToolCallMessagePartComponent = ({ args, toolCallId }) => {
  const { t } = useT();
  const threadId = useAuiThreadId();
  const steps = Array.isArray((args as { steps?: unknown } | undefined)?.steps)
    ? ((args as { steps: unknown[] }).steps.filter((s): s is string => typeof s === 'string') as string[])
    : [];
  const pending = useAppSelector(state =>
    threadId ? state.chatRuntime.pendingPlanReviewByThread[threadId] ?? null : null
  );
  const isForThisCall =
    pending != null && (pending.toolCallId ? pending.toolCallId === toolCallId : true);

  if (!threadId || steps.length === 0) return null;

  if (isForThisCall && pending) {
    return <PlanReviewCardCore threadId={threadId} review={pending} />;
  }

  return <AgentPlan steps={steps} activeIndex={steps.length} title={t('conversations.planReview.title')} />;
};
