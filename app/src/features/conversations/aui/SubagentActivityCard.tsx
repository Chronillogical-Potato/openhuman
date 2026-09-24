'use client';

/**
 * Renders a bare {@link SubagentActivity} (not wrapped in an assistant-ui
 * message part) through the vendored `elements/task-card.tsx` primitives —
 * the same `TaskCard` + `TaskTranscript` pairing `SubagentTaskCard.tsx` uses
 * for a live `task` tool-call part.
 *
 * `SubagentTaskCard` cannot be reused directly here: it is a
 * `ToolCallMessagePartComponent` that reads `args`/`result`/`messages` off an
 * assistant-ui part, and it wires the awaiting-user reply box through
 * `useAui()` — which requires an ambient `AssistantRuntimeProvider`. This
 * component's callers (`ToolTimelineAdapter`, `AgentProcessSourcePanel`) can
 * render outside that provider (e.g. `TranscriptOverlays` is a sibling of
 * `AssistantUiChat`, not a descendant of it), so this stays read-only: the
 * awaiting-user question is shown as text with no reply box, and there is no
 * "view full processing" drawer affordance — the nested transcript is always
 * inline via `TaskCard`'s own disclosure, mirroring what `SubagentTaskCard`
 * does for a live delegation.
 */
import { useT } from '../../../lib/i18n/I18nContext';
import { subagentMessages } from '../../../providers/assistantUiMessages';
import { isActiveTimelineStatus, type SubagentActivity } from '../../../store/chatRuntimeSlice';
import { basename } from '../../../utils/pathUtils';
import { TaskCard, type TaskCardState } from '../../../components/assistant-ui/elements/task-card';
import { TaskTranscript } from '../../../components/assistant-ui/elements/task-card.aui';
import { formatElapsed } from '../../../components/assistant-ui/utils/task';
import Badge from '../../../components/ui/Badge';
import WorktreeActions from '../../../components/worktree/WorktreeActions';

function stateOf(activity: SubagentActivity): TaskCardState {
  if (activity.status === 'awaiting_user') return 'waiting';
  if (isActiveTimelineStatus(activity.status)) return 'working';
  if (activity.status === 'failed') return 'failed';
  if (activity.status === 'cancelled') return 'cancelled';
  return 'done';
}

function WorktreeRow({ activity }: { activity: SubagentActivity }) {
  const { t } = useT();
  if (!activity.worktreePath) return null;
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="font-medium text-content-secondary">{t('worktree.label')}</span>
        <span
          className="truncate font-mono text-[12px] text-content-muted"
          title={activity.worktreePath}>
          {basename(activity.worktreePath)}
        </span>
        <Badge variant={activity.isDirty ? 'warning' : 'success'} className="rounded-full">
          {activity.isDirty ? t('worktree.dirty') : t('worktree.clean')}
        </Badge>
      </div>
      <WorktreeActions path={activity.worktreePath} isDirty={activity.isDirty} compact />
    </div>
  );
}

export function SubagentActivityCard({ activity }: { activity: SubagentActivity }) {
  const { t } = useT();
  const state = stateOf(activity);
  const name = activity.displayName ?? activity.agentId ?? 'subagent';
  const elapsed = activity.elapsedMs !== undefined ? formatElapsed(activity.elapsedMs) : undefined;
  const awaiting = state === 'waiting';

  const actions =
    awaiting || activity.worktreePath ? (
      <div className="flex flex-col gap-2.5">
        {awaiting ? (
          <div data-testid="subagent-awaiting-user" className="flex flex-col gap-1.5">
            <p className="text-[12px] font-medium text-amber-800 dark:text-amber-200">
              {t('conversations.subagent.awaitingTitle')}
            </p>
            {activity.awaitingQuestion ? (
              <p
                data-testid="subagent-awaiting-question"
                className="wrap-break-word whitespace-pre-wrap text-[12px] text-content-secondary">
                {activity.awaitingQuestion}
              </p>
            ) : null}
          </div>
        ) : null}
        <WorktreeRow activity={activity} />
      </div>
    ) : undefined;

  const resultNode =
    activity.output && (state === 'done' || state === 'failed') ? (
      <p className="m-0 whitespace-pre-wrap">{activity.output}</p>
    ) : undefined;

  const messages = subagentMessages(activity);

  return (
    <TaskCard
      data-testid="assistant-ui-subagent-call"
      data-status={activity.status ?? state}
      label={`${t('conversations.tools.delegatedTo').replace('{agent}', name)}`}
      meta={activity.mode}
      state={state}
      elapsed={elapsed}
      actions={actions}
      result={resultNode}>
      {messages.length > 0 ? (
        <div data-testid="subagent-activity">
          <TaskTranscript messages={messages} />
        </div>
      ) : undefined}
    </TaskCard>
  );
}

export default SubagentActivityCard;
