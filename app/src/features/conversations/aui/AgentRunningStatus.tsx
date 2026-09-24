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
 * With no task running, this falls back to assistant-ui's vendored
 * `GenerationLoader`, with a shimmering Thinking label, so a turn that has not
 * yet spawned a sub-agent still shows one coherent running signal.
 */
import { useEffect, useState } from 'react';

import {
  TaskTray,
  useTaskSummary,
} from '../../../components/assistant-ui/elements/agent-status.aui';
import { GenerationLoader } from '../../../components/assistant-ui/elements/loading-state';
import { useT } from '../../../lib/i18n/I18nContext';

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
  const { t } = useT();
  const summary = useTaskSummary();
  const strings = useAgentStatusStrings();
  const [tick, setTick] = useState(0);

  useEffect(() => {
    if (summary.total > 0) return undefined;
    const timer = window.setInterval(() => setTick(value => value + 1), 120);
    return () => window.clearInterval(timer);
  }, [summary.total]);

  if (summary.total === 0) {
    return (
      <GenerationLoader
        data-testid="agent-running-status-thinking"
        label={t('chat.thinkingDots')}
        tick={tick}
        variant="rounded"
        className="flex-row justify-start gap-2.5 px-2 [&>div]:gap-0.5 [&>div>span]:size-1"
      />
    );
  }
  return <TaskTray data-testid="agent-running-status-tasks" strings={strings} />;
}

export default AgentRunningStatus;
