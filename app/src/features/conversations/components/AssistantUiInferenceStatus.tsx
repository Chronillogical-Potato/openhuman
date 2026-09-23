import { type AssistantState, useAuiState } from '@assistant-ui/react';

import { readOpenHumanThreadExtras } from '../../../providers/useOpenHumanExternalStore';
import { InferenceStatusLine } from './aui/InferenceStatusLine';

const selectThreadExtras = (state: AssistantState) => state.thread.extras;

/**
 * The "what is the model doing right now" line, on the assistant-ui surface.
 *
 * assistant-ui knows only `thread.isRunning`, so before this the whole of
 * `chatRuntime.inferenceStatusByThread` — reasoning round, active tool,
 * delegated sub-agent — stopped at `ChatThreadView`, which `/chat` no longer
 * renders. A slow turn was an unlabelled spinner.
 *
 * The status arrives on the runtime's `extras` channel
 * (`useOpenHumanExternalStore`), so it is always the status of the thread *this
 * runtime* represents; this component holds no Redux read of its own and is
 * therefore safe on a second runtime such as the Workflow Copilot's.
 *
 * Rendering is shared with the legacy surface (`InferenceStatusLine`) so the
 * mic-cloud composer and `/chat` cannot drift apart, and so is the rule for
 * when to show it: the `tool_use` / `subagent` phases only restate the running
 * row, which this surface already paints as a tool part, so the line would be
 * a duplicate caption under the card. It is kept as a fallback whenever the
 * phase's row is not on screen (a restored snapshot, or a row that settled
 * ahead of the status), where `status.activeTool` / `status.activeSubagent` is
 * the only name for the work in flight.
 *
 * The `thinking` phase renders NOTHING here. It used to show
 * `Thinking... (N)` — a pulsing dot plus the harness's iteration counter — but
 * assistant-ui already marks a running turn as in flight, so ours was a second
 * indicator stacked under it, and the iteration count was harness telemetry the
 * reader has no use for.
 *
 * The library's marker is a *message-level* part, which is the detail that
 * matters here. While the thread is running assistant-ui mints a placeholder
 * assistant message; `MessagePrimitive.GroupedParts` emits a synthetic
 * `indicator` part for a running message with zero content parts
 * (`@assistant-ui/core`, `contentLength === 0 && isRunning`), and
 * `components/assistant-ui/thread.tsx` renders that part as
 * `<span data-slot="aui_assistant-message-indicator">●</span>`. Being
 * message-level, it exists only on this surface — `ChatThreadView` has no
 * equivalent, which is why the shared `InferenceStatusLine` keeps its
 * `thinking` branch and the suppression lives here instead.
 *
 * It is NOT `@assistant-ui/react-markdown/styles/dot.css` painting
 * `.aui-md[data-status="running"]:empty::after`. That rule cannot fire in this
 * window: `assistantParts` pushes a text part only when `text.length > 0` and
 * `streamingTailMessage` returns `null` at `parts.length === 0`
 * (`providers/assistantUiMessages.ts`), so no `.aui-md` element exists yet for
 * `:empty` to match. Said here because believing the marker was CSS on a
 * markdown element is what once made deleting the shared `thinking` branch look
 * safe; it blanked the voice surface.
 */
export function AssistantUiInferenceStatus() {
  const extras = readOpenHumanThreadExtras(useAuiState(selectThreadExtras));
  const status = extras?.inferenceStatus;
  if (!status) return null;
  if (status.phase === 'thinking') return null;

  const activeRow =
    status.phase === 'subagent'
      ? extras?.activeSubagentEntry
      : status.phase === 'tool_use'
        ? extras?.activeToolEntry
        : undefined;
  if (activeRow) return null;

  return (
    <InferenceStatusLine
      status={status}
      activeToolEntry={extras?.activeToolEntry}
      activeSubagentEntry={extras?.activeSubagentEntry}
    />
  );
}

export default AssistantUiInferenceStatus;
