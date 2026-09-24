import type { ToolCallMessagePartComponent } from '@assistant-ui/react';

import { mono } from '../../../components/assistant-ui/elements/surfaces';
import { useT } from '../../../lib/i18n/I18nContext';

/**
 * Compact one-line summary for the `goal_set` / `goal_get` / `goal_complete`
 * tool calls — "{objective} ({status})" — rendered inline in the activity
 * trace rather than as a bespoke card. The pinned, always-current goal above
 * the composer is a separate render (`AgentStatus` fed by `useThreadGoal`),
 * driven by the `thread_goal_updated` event, not this per-call snapshot.
 *
 * `@assistant-ui/react` 0.15.16 has no toolkit-level `renderText` field for a
 * one-line-only entry, so this is an ordinary `render` that happens to be a
 * single text row — the closest approximation available.
 */
interface GoalToolPayload {
  goal?: {
    objective?: string;
    status?: string;
  } | null;
}

export const GoalToolLine: ToolCallMessagePartComponent = ({ args, result }) => {
  const { t } = useT();
  const payload = (result ?? args) as GoalToolPayload | undefined;
  const goal = payload?.goal;
  if (!goal || typeof goal.objective !== 'string' || typeof goal.status !== 'string') return null;
  const line = t('conversations.goal.inlineSummary')
    .replace('{objective}', goal.objective)
    .replace('{status}', goal.status);
  return <span className={mono}>{line}</span>;
};
