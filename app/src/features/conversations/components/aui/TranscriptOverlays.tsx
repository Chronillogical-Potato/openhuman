import { useState } from 'react';

import type {
  ProcessingTranscriptItem,
  ToolTimelineEntry,
} from '../../../../store/chatRuntimeSlice';
import { AgentProcessSourcePanel } from '../AgentProcessSourcePanel';
import { type BackgroundProcess, BackgroundProcessesPanel } from '../BackgroundProcessesPanel';

export interface TranscriptOverlaysProps {
  threadId: string | null;
  /** The thread's full live tool timeline — the Agent Process Source corpus. */
  entries: ToolTimelineEntry[];
  /** The thread's live reasoning/narration trail. */
  transcript: ProcessingTranscriptItem[];
  backgroundProcesses: BackgroundProcess[];
  showBackgroundProcesses: boolean;
  onCloseBackgroundProcesses: () => void;
  showProcessSource: boolean;
  /** Scopes the process-source panel to one step; `undefined` = whole run. */
  scopedEntry?: ToolTimelineEntry;
  onCloseProcessSource: () => void;
}

/**
 * The transcript-local overlays: background sub-agents, and the Agent
 * Process Source panel.
 *
 * Mounted beside the assistant-ui `Thread` by each host (the home chat and the
 * workflow copilot) because none of it is part of the transcript's render path
 * — it is driven entirely by the host's own disclosure state.
 *
 * The dedicated sub-agent drawer (`SubagentDrawer`) is gone: a delegation's
 * nested activity now always renders inline through its own `TaskCard`
 * disclosure (`SubagentTaskCard` for a live `task` part, `SubagentActivityCard`
 * for a bare `SubagentActivity` here), mirroring what already shipped for the
 * `task` toolkit entry. Clicking a background process now opens the whole-run
 * Agent Process Source panel scoped to that task's step instead of a
 * dedicated drawer.
 *
 * Known gap: the drawer used to offer a "Cancel task" affordance for a still-
 * running detached (`async`) sub-agent, backed by `subagentApi.cancel`. Neither
 * `SubagentTaskCard` nor `SubagentActivityCard` exposes an equivalent action —
 * there is currently no UI to cancel a running background task. Filed as a
 * product gap rather than invented here.
 */
export function TranscriptOverlays({
  threadId: _threadId,
  entries,
  transcript,
  backgroundProcesses,
  showBackgroundProcesses,
  onCloseBackgroundProcesses,
  showProcessSource,
  scopedEntry,
  onCloseProcessSource,
}: TranscriptOverlaysProps) {
  // A background process opened from its own panel scopes the Agent Process
  // Source panel to that task's step, without disturbing the caller's own
  // whole-run `showProcessSource` toggle (the command palette's "Open agent
  // process source" action).
  const [scopedTaskId, setScopedTaskId] = useState<string | null>(null);
  const backgroundScopedEntry = scopedTaskId
    ? entries.find(entry => entry.subagent?.taskId === scopedTaskId)
    : undefined;
  const effectiveOpen = showProcessSource || backgroundScopedEntry !== undefined;
  const effectiveScopedEntry = backgroundScopedEntry ?? scopedEntry;

  return (
    <>
      <BackgroundProcessesPanel
        open={showBackgroundProcesses}
        processes={backgroundProcesses}
        onClose={onCloseBackgroundProcesses}
        onOpenProcess={taskId => {
          onCloseBackgroundProcesses();
          setScopedTaskId(taskId);
        }}
      />
      <AgentProcessSourcePanel
        open={effectiveOpen}
        entries={entries}
        transcript={transcript}
        scopedEntry={effectiveScopedEntry}
        onClose={() => {
          setScopedTaskId(null);
          onCloseProcessSource();
        }}
      />
    </>
  );
}
