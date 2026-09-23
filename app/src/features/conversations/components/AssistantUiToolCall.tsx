import type { ToolCallMessagePart, ToolCallMessagePartProps } from '@assistant-ui/react';
import { CheckIcon, ChevronDownIcon, CircleXIcon, Loader2Icon } from 'lucide-react';
import type { FC, ReactNode } from 'react';

import { cn } from '../../../components/assistant-ui/lib/utils';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '../../../components/assistant-ui/ui/collapsible';
import { useT } from '../../../lib/i18n/I18nContext';
import { readOpenHumanToolArtifact } from '../../../providers/assistantUiMessages';
import type {
  ToolFailureExplanation,
  ToolTimelineEntryStatus,
} from '../../../store/chatRuntimeSlice';
import { FetchBody, FileBody, ShellBody, WebSearchResults } from '../tools/ToolBodies';
import { hasDisplayValue, parsedValue, ToolDataView } from '../tools/ToolDataView';
import { ToolIcon } from '../tools/ToolIcon';
import {
  describeToolCall,
  parseToolArgs,
  type ToolCallPresentation,
  toolLabel,
} from '../tools/toolPresentation';
import { ToolFailureLines } from './ToolFailureLines';

/** `1234` → "1.2s", `850` → "850ms", `75000` → "1m 15s". */
export function formatElapsed(ms: number): string {
  if (ms < 1000) return `${Math.max(0, Math.round(ms))}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const minutes = Math.floor(ms / 60_000);
  const seconds = Math.round((ms % 60_000) / 1000);
  return `${minutes}m ${seconds}s`;
}

/**
 * Is this approval still the user's to answer?
 *
 * A resolved one keeps its `approval` object (with `approved` / `resolution`
 * filled in) so the transcript can show what was decided — offering the buttons
 * again would let a second decision race the first.
 */
export function isApprovalPending(approval: ToolCallMessagePart['approval']): boolean {
  return approval != null && approval.approved === undefined && approval.resolution === undefined;
}

/**
 * A step's node on the timeline rail: the tool's icon in a ring that sits on
 * the vertical line `ToolTimeline` draws. Shared with the delegation card so
 * every step in a group lines up.
 */
export function TimelineNode({
  presentation,
  state,
}: {
  presentation: Pick<ToolCallPresentation, 'icon' | 'integration'>;
  state: 'running' | 'done' | 'failed' | 'awaiting';
}) {
  return (
    <span
      aria-hidden
      data-slot="tool-timeline-node"
      className={cn(
        'bg-background absolute top-1.5 left-0 z-10 flex size-6 items-center justify-center rounded-full ring-1',
        state === 'running' && 'ring-primary/50 text-foreground',
        state === 'done' && 'ring-border text-muted-foreground',
        state === 'failed' && 'text-red-600 ring-red-500/40 dark:text-red-400',
        state === 'awaiting' && 'text-amber-700 ring-amber-400/60 dark:text-amber-300'
      )}>
      <ToolIcon presentation={presentation} className="size-3.5" />
    </span>
  );
}

export interface AssistantUiToolCallCardProps {
  toolName: string;
  args?: unknown;
  argsText?: string;
  result?: unknown;
  status?: ToolTimelineEntryStatus;
  /** Server label; used only for tools the presentation registry cannot describe. */
  displayName?: string;
  detail?: string;
  elapsedMs?: number;
  /** Machine-readable result from the core (e.g. structured web-search hits). */
  structured?: unknown;
  failure?: ToolFailureExplanation;
  /**
   * The call is parked on the user — an ApprovalGate request, or a sub-agent
   * that asked a question. Renders as in-flight (it *is* unfinished) but says
   * what it is actually waiting for.
   */
  awaitingUser?: boolean;
  /** Decision row / connect affordance, rendered under the call's header. */
  footer?: ReactNode;
}

function ToolBody({
  presentation,
  args,
  result,
  running,
}: {
  presentation: ToolCallPresentation;
  args: Record<string, unknown>;
  result: unknown;
  running: boolean;
}): ReactNode {
  if (running) return null;
  switch (presentation.body) {
    case 'shell':
      return <ShellBody args={args} result={result} />;
    case 'webFetch':
      return <FetchBody args={args} result={result} />;
    case 'file':
      return <FileBody args={args} result={result} />;
    default:
      return null;
  }
}

/**
 * One tool call, rendered as a step on the tool timeline.
 *
 * The icon, the label and the target chip all come from the presentation
 * registry, so every surface names a call the same way. The label changes
 * tense as the call settles ("Reading file" → "Read file"), and the step
 * expands into the tool's own renderer: search results, a terminal, a diff,
 * a fetched page, or the generic Input/Output view.
 */
export function AssistantUiToolCallCard({
  toolName,
  args,
  argsText,
  result,
  status,
  displayName,
  detail,
  elapsedMs,
  structured,
  failure,
  awaitingUser = false,
  footer,
}: AssistantUiToolCallCardProps) {
  const { t } = useT();
  const running =
    awaitingUser ||
    (status ? status === 'running' || status === 'awaiting_user' : result === undefined);
  const effectiveStatus: ToolTimelineEntryStatus =
    status ?? (awaitingUser ? 'awaiting_user' : running ? 'running' : 'success');
  const input = hasDisplayValue(args) ? args : parsedValue(argsText ?? '');
  const parsedArgs = parseToolArgs(input);
  const output = result === '' && status && !running ? t('conversations.tools.noOutput') : result;
  const presentation = describeToolCall({
    name: toolName,
    args: parsedArgs,
    status: effectiveStatus,
    serverLabel: displayName,
    serverDetail: detail,
  });
  const label = toolLabel(presentation, t);
  const failed = status === 'error';
  // A cancelled call did not succeed either: it gets the failure icon, not a
  // check, even though only an `error` carries an explanation block.
  const terminalNonSuccess = failed || status === 'cancelled';
  const awaiting = awaitingUser || status === 'awaiting_user';
  const statusLabel = failed
    ? t('conversations.tools.status.failed')
    : status === 'cancelled'
      ? t('conversations.tools.status.cancelled')
      : awaiting
        ? t('conversations.tools.status.awaiting')
        : running
          ? t('conversations.tools.status.running')
          : t('conversations.tools.status.done');
  const nodeState = terminalNonSuccess
    ? 'failed'
    : awaiting
      ? 'awaiting'
      : running
        ? 'running'
        : 'done';
  const richBody = ToolBody({ presentation, args: parsedArgs, result: output, running });
  const isSearch = presentation.body === 'webSearch';
  const searchBody = isSearch ? (
    <WebSearchResults
      args={parsedArgs}
      result={output}
      structured={structured}
      searching={running}
    />
  ) : null;

  return (
    <Collapsible
      data-slot="aui_openhuman-tool-call"
      data-testid="assistant-ui-tool-call"
      data-tool={presentation.baseName}
      data-status={effectiveStatus}
      data-awaiting-user={awaitingUser ? 'true' : undefined}
      defaultOpen={awaitingUser}
      className="group/step relative min-w-0 pl-9">
      <TimelineNode presentation={presentation} state={nodeState} />
      <CollapsibleTrigger className="group/tool text-muted-foreground hover:text-foreground flex w-full min-w-0 items-center gap-2 py-1.5 text-sm transition-colors">
        <span
          data-testid="tool-call-label"
          className={cn(
            'text-foreground shrink-0 text-start font-medium',
            running && !awaiting && 'tool-shimmer'
          )}>
          {label}
        </span>
        {presentation.chip ? (
          <span
            data-testid="tool-call-chip"
            className="bg-muted text-muted-foreground min-w-0 truncate rounded-md px-1.5 py-0.5 font-mono text-[11px]">
            {presentation.chip}
          </span>
        ) : null}
        <span className="ml-auto flex shrink-0 items-center gap-1.5 text-[11px]">
          {running && !awaiting ? (
            <Loader2Icon className="size-3 animate-spin [animation-duration:0.6s]" />
          ) : terminalNonSuccess ? (
            <CircleXIcon className="size-3.5 text-red-600 dark:text-red-400" />
          ) : awaiting ? null : (
            <CheckIcon className="size-3.5 text-emerald-600 dark:text-emerald-400" />
          )}
          <span
            data-testid="tool-call-status"
            className={cn(
              (running && !awaiting) || (!running && !terminalNonSuccess) ? 'sr-only' : undefined,
              awaiting && 'text-amber-700 dark:text-amber-300'
            )}>
            {statusLabel}
          </span>
          {elapsedMs != null && !running ? (
            <span data-testid="tool-call-elapsed" className="tabular-nums">
              {formatElapsed(elapsedMs)}
            </span>
          ) : null}
          <ChevronDownIcon className="size-4 shrink-0 -rotate-90 transition-transform group-data-[state=open]/tool:rotate-0" />
        </span>
      </CollapsibleTrigger>
      {failed && failure ? (
        <div className="pb-2">
          <ToolFailureLines failure={failure} />
        </div>
      ) : null}
      {/* Outside `CollapsibleContent` on purpose: a decision the turn is
          blocked on must not be hidden behind a disclosure the user has to
          find and open. Search results are the call's whole point, so they
          stay visible too. */}
      {footer}
      {searchBody ? <div className="pt-0.5 pb-2">{searchBody}</div> : null}
      <CollapsibleContent className="space-y-2 pb-3">
        {richBody}
        {!richBody && hasDisplayValue(input) ? (
          <div data-testid="assistant-ui-tool-input">
            <p className="text-muted-foreground mb-1 text-[11px] font-medium uppercase">
              {t('conversations.subagent.input')}
            </p>
            <div className="max-h-48 overflow-auto">
              <ToolDataView value={input} />
            </div>
          </div>
        ) : null}
        {!richBody && !searchBody && hasDisplayValue(parsedValue(output)) ? (
          <div data-testid="assistant-ui-tool-output">
            <p className="text-muted-foreground mb-1 text-[11px] font-medium uppercase">
              {t('conversations.subagent.output')}
            </p>
            <div className="max-h-64 overflow-auto">
              <ToolDataView value={output} />
            </div>
          </div>
        ) : null}
      </CollapsibleContent>
    </Collapsible>
  );
}

/**
 * Terminal status carried inside a settled tool part's `result`.
 *
 * assistant-ui's tool-call part has no status field, so `toolPart` puts the
 * status there for a tool that failed or was cancelled (`value` holds the real
 * output when there was one). Without unwrapping it here the card fell back to
 * `result !== undefined`, which reads as success — a failed tool rendered
 * "done" with a check.
 */
function toolStatusEnvelope(
  result: unknown
):
  | { status: ToolTimelineEntryStatus; failure?: ToolFailureExplanation; value?: unknown }
  | undefined {
  if (!result || typeof result !== 'object' || Array.isArray(result)) return undefined;
  const candidate = result as { status?: unknown; failure?: unknown; value?: unknown };
  return candidate.status === 'error' || candidate.status === 'cancelled'
    ? {
        status: candidate.status as ToolTimelineEntryStatus,
        failure: candidate.failure as ToolFailureExplanation | undefined,
        ...('value' in candidate ? { value: candidate.value } : {}),
      }
    : undefined;
}

/**
 * One tool call in the assistant-ui transcript.
 *
 * `approval` is supplied by assistant-ui on every tool part; this component used
 * to destructure four fields and drop the rest, which is why a parked call
 * rendered as an ordinary running one with no way to answer it.
 *
 * The part's `artifact` carries what the core said about the call (its label
 * for a dynamic tool, the duration, a structured result). The adapter used to
 * drop all of it, so the card guessed a label from the tool name.
 *
 * The decision surface itself is passed in rather than built here. It is
 * `ApprovalRequestCard`, which needs the thread id and the store's
 * `PendingApproval` — neither of which belongs in this file, and both of which
 * `ChatToolParts` already resolves for the `composio_connect` route.
 */
export const OpenHumanToolCall: FC<
  ToolCallMessagePartProps & {
    /** Decision surface for a parked call; rendered under the call's header. */
    approvalCard?: ReactNode;
  }
> = props => {
  const envelope = toolStatusEnvelope(props.result);
  const artifact = readOpenHumanToolArtifact(props.artifact);
  return (
    <AssistantUiToolCallCard
      toolName={props.toolName}
      args={props.args}
      argsText={props.argsText}
      result={envelope ? envelope.value : props.result}
      status={envelope?.status}
      failure={envelope?.failure}
      displayName={artifact?.displayName}
      detail={artifact?.detail}
      elapsedMs={artifact?.elapsedMs}
      structured={artifact?.structured}
      awaitingUser={isApprovalPending(props.approval)}
      footer={props.approvalCard}
    />
  );
};
