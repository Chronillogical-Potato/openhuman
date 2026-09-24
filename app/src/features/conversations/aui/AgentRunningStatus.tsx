'use client';

/**
 * The `Thread`'s `RunningStatus` slot (`components/assistant-ui/thread.tsx`),
 * on the assistant-ui surface. Replaces `AssistantUiInferenceStatus.tsx` /
 * `aui/InferenceStatusLine.tsx` (deleted).
 *
 * Where those read `chatRuntime.inferenceStatusByThread` (phase/active tool/
 * active subagent) through the runtime's `extras` channel, this reads
 * assistant-ui's own `s.thread.tasks` (`elements/agent-status.aui.tsx`'s
 * `TaskTray`) — the delegations the `task` toolkit entry registered as nested
 * tasks (`providers/assistantUiMessages.ts`'s `subagentMessages`). A plain
 * tool call (read a file, run a shell command, web search) is not a "task" by
 * that definition — it has no nested transcript — so it never reaches this
 * component at all; assistant-ui's own running-message indicator already
 * signals "something is happening" for those, same as before.
 *
 * With no task running, this falls back to the vendored `ThinkingIndicator`
 * (WS-E) rather than rendering nothing, so a turn that has not yet spawned any
 * sub-agent still shows a running signal beneath the composer.
 */
import { TaskTray } from '../../../components/assistant-ui/elements/agent-status.aui';
import { ThinkingIndicator } from '../../../components/assistant-ui/elements/thinking-indicator';
import { useT } from '../../../lib/i18n/I18nContext';
import { useTaskSummary } from '../../../components/assistant-ui/elements/agent-status.aui';

/** English defaults mapped onto `AgentStatusStrings` via `useT()`. */
function useAgentStatusStrings() {
  const { t } = useT();
  return {
    taskOne: t('conversations.tasks.taskOne'),
    taskOther: t('conversations.tasks.taskOther'),
    running: t('conversations.tasks.running'),
    waitingForInput: t('conversations.tasks.waitingForInput'),
    done: t('conversations.tasks.done'),
    failed: t('conversations.tasks.failed'),
    of: t('conversations.tasks.of'),
  };
}

export function AgentRunningStatus() {
  const summary = useTaskSummary();
  const strings = useAgentStatusStrings();
  if (summary.total === 0) {
    return <ThinkingIndicator data-testid="agent-running-status-thinking" />;
  }
  return <TaskTray data-testid="agent-running-status-tasks" strings={strings} />;
}

export default AgentRunningStatus;
