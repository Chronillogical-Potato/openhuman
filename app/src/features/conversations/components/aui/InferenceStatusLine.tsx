import type { ToolTimelineEntry } from '../../../../store/chatRuntimeSlice';
import { formatTimelineEntry } from '../../../../utils/toolTimelineFormatting';

export interface InferenceStatusLineProps {
  status: {
    phase: 'thinking' | 'tool_use' | 'subagent' | string;
    iteration: number;
    activeTool?: string;
    activeSubagent?: string;
  };
  /** The newest running non-subagent row, used to title the `tool_use` phase. */
  activeToolEntry?: ToolTimelineEntry;
  /** The running subagent row, used to title the `subagent` phase. */
  activeSubagentEntry?: ToolTimelineEntry;
}

/**
 * The one-line "what is the model doing right now" indicator.
 *
 * For the `tool_use` / `subagent` phases this only restates the active row the
 * agentic-task-insights timeline already shows, so the caller suppresses it
 * once that timeline is on screen, and keeps it when there is no timeline row
 * to fall back on.
 *
 * There is deliberately NO `thinking` branch. It rendered `Thinking... (N)`,
 * which stacked a second indicator under assistant-ui's own: the library's
 * `react-markdown/styles/dot.css` paints a pulsing `●` on
 * `.aui-md[data-status="running"]:empty`, which is live for precisely the
 * pre-first-token gap this line was covering. The `(N)` was the harness's
 * iteration counter — internal telemetry, not something a reader can act on.
 * `AssistantUiInferenceStatus` returns null for that phase before reaching
 * here; this component has no translation for it either.
 */
export function InferenceStatusLine({
  status,
  activeToolEntry,
  activeSubagentEntry,
}: InferenceStatusLineProps) {
  // No `useT` here any more: the only translated strings this component had
  // were the `thinking` ones, and both keys are gone. The `tool_use` /
  // `subagent` captions come from `formatTimelineEntry`, which localises the
  // row title itself.
  return (
    <div
      data-testid="inference-status-line"
      className="flex items-center gap-2 px-1 py-1.5 text-xs text-content-muted">
      <span className="inline-block w-2 h-2 rounded-full bg-primary-400 animate-pulse" />
      <span>
        {status.phase === 'tool_use' &&
          `${
            formatTimelineEntry(
              activeToolEntry ?? {
                id: 'active-tool',
                name: status.activeTool ?? 'tool',
                round: status.iteration,
                seq: 0,
                status: 'running',
              }
            ).title
          }...`}
        {status.phase === 'subagent' &&
          `${
            formatTimelineEntry(
              activeSubagentEntry ?? {
                id: 'active-subagent',
                name: `subagent:${status.activeSubagent ?? ''}`,
                round: status.iteration,
                seq: 0,
                status: 'running',
              }
            ).title
          }...`}
      </span>
    </div>
  );
}
