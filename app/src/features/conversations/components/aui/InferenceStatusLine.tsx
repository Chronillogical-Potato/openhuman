import { useT } from '../../../../lib/i18n/I18nContext';
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
 * The `thinking` branch renders `Thinking... (N)`, and stays here because this
 * component is shared: `ChatThreadView` (the legacy / voice-mode surface) renders
 * this line *specifically* for that phase, and that surface has no assistant-ui
 * markdown, so nothing else indicates the pre-first-token gap for it.
 *
 * On the assistant-ui surface that caption WOULD stack a second indicator under
 * the library's own, and the `(N)` is the harness's iteration counter, internal
 * telemetry a reader cannot act on. That is suppressed one layer up:
 * `AssistantUiInferenceStatus` returns null for `thinking` before reaching this
 * component — see its doc comment for what the library's marker actually is (a
 * message-level synthetic `indicator` part, not a CSS rule on `.aui-md`) and why
 * that is precisely why this surface has no equivalent.
 *
 * So the suppression is per-surface, at the surface-specific caller, rather than
 * by deleting a branch the other caller depends on.
 */
export function InferenceStatusLine({
  status,
  activeToolEntry,
  activeSubagentEntry,
}: InferenceStatusLineProps) {
  // The `thinking` caption stays HERE, in the shared component, because the
  // legacy surface still needs it: `ChatThreadView` renders this line
  // specifically for that phase ("keep it only for the `thinking` phase, which
  // has no timeline row yet") and has no assistant-ui markdown, so nothing
  // paints a dot for it. Removing the branch left that surface with a bare
  // pulse and no caption.
  //
  // The duplicate this PR is fixing is an assistant-ui problem, and it is fixed
  // at the assistant-ui layer: `AssistantUiInferenceStatus` returns null for
  // `thinking` before it ever reaches this component, so the caption below
  // cannot stack under assistant-ui's own `●` there — a message-level
  // `indicator` part that has no counterpart on this surface.
  const { t } = useT();
  return (
    <div
      data-testid="inference-status-line"
      className="flex items-center gap-2 px-1 py-1.5 text-xs text-content-muted">
      <span className="inline-block w-2 h-2 rounded-full bg-primary-400 animate-pulse" />
      <span>
        {status.phase === 'thinking' &&
          (status.iteration > 0
            ? t('chat.thinkingIteration').replace('{n}', String(status.iteration))
            : t('chat.thinkingDots'))}
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
