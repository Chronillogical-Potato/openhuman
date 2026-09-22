import React, { useState } from 'react';
import { LuTarget } from 'react-icons/lu';

import Badge, { type BadgeVariant } from '../../../components/ui/Badge';
import { cn } from '../../../lib/cn';
import { useT } from '../../../lib/i18n/I18nContext';
import type { ThreadGoalStatus, ThreadGoalView } from '../utils/harnessState';

/**
 * The thread's goal — the durable objective the agent set with `goal_set`
 * and keeps pursuing across turns — pinned above the composer as a
 * read-only strip: status pill, the objective, and token usage against the
 * budget when one was set. See {@link selectThreadGoal} for where it comes
 * from. A long objective clamps to one line and expands on click.
 */
interface Props {
  goal: ThreadGoalView;
}

const STATUS_KEY: Record<ThreadGoalStatus, string> = {
  active: 'conversations.goal.status.active',
  paused: 'conversations.goal.status.paused',
  budget_limited: 'conversations.goal.status.budgetLimited',
  complete: 'conversations.goal.status.complete',
};

const STATUS_VARIANT: Record<ThreadGoalStatus, BadgeVariant> = {
  active: 'primary',
  paused: 'neutral',
  budget_limited: 'warning',
  complete: 'success',
};

/** `1234` → `1.2k`, `2500000` → `2.5M`; small counts stay exact. */
export function formatTokens(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(1).replace(/\.0$/, '')}k`;
  return String(count);
}

export const GoalBanner: React.FC<Props> = ({ goal }) => {
  const { t } = useT();
  const [expanded, setExpanded] = useState(false);
  const usage =
    goal.tokenBudget !== null
      ? t('conversations.goal.tokensWithBudget')
          .replace('{used}', formatTokens(goal.tokensUsed))
          .replace('{budget}', formatTokens(goal.tokenBudget))
      : t('conversations.goal.tokens').replace('{used}', formatTokens(goal.tokensUsed));

  return (
    <section
      aria-label={t('conversations.goal.title')}
      data-testid="goal-banner"
      data-goal-status={goal.status}
      className={cn(
        'mb-2 flex items-start gap-2 rounded-xl border bg-surface px-3 py-2 text-sm shadow-sm',
        goal.status === 'complete'
          ? 'border-sage-200 dark:border-sage-500/30'
          : 'border-primary-200 dark:border-primary-500/30'
      )}>
      <LuTarget
        aria-hidden
        className="mt-0.5 h-4 w-4 shrink-0 text-primary-700 dark:text-primary-200"
      />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-semibold text-content">{t('conversations.goal.title')}</span>
          <Badge variant={STATUS_VARIANT[goal.status]} data-testid="goal-status">
            {t(STATUS_KEY[goal.status])}
          </Badge>
          <span className="ml-auto text-xs text-content-secondary" data-testid="goal-tokens">
            {usage}
          </span>
        </div>
        <button
          type="button"
          data-analytics-id="goal-banner-toggle"
          aria-expanded={expanded}
          onClick={() => setExpanded(prev => !prev)}
          className={cn(
            'mt-1 w-full text-left wrap-break-word text-content-secondary',
            !expanded && 'line-clamp-1'
          )}
          data-testid="goal-objective">
          {goal.objective}
        </button>
      </div>
    </section>
  );
};

export default GoalBanner;
