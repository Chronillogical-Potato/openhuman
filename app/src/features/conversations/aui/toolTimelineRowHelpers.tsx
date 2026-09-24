import type { ReactNode } from 'react';

import Badge from '../../../components/ui/Badge';
import type { ToolTimelineEntry, ToolTimelineEntryStatus } from '../../../store/chatRuntimeSlice';
import { formatTimelineEntry } from '../../../utils/toolTimelineFormatting';
import type { WorkerThreadStatus } from '../components/WorkerThreadRefCard';

/**
 * Row-level presentational helpers shared by {@link ToolTimelineAdapter}
 * (`ToolTimelineAdapter.tsx`, the vendored `tool-timeline` element's OpenHuman
 * host) and {@link AgentProcessSourcePanel}. Folded together from the deleted
 * `components/toolTimelineRows.tsx` and `components/AgentTimelineRail.tsx` —
 * both were pure presentation with no behavior of their own, so nothing here
 * changed except the import paths.
 */

/**
 * Map a parent timeline entry's status to the worker-thread lifecycle phase
 * rendered on `WorkerThreadRefCard`. The parent entry is what the
 * subagent_spawned / subagent_completed / subagent_failed socket events
 * mutate, so reading from it keeps the badge and the surrounding
 * disclosure's status pill in lockstep without a second source of truth.
 *
 * Returns `undefined` for the rare ambiguous case so the card stays
 * label-only rather than render a misleading state.
 */
export function workerStatusFromEntry(
  status: ToolTimelineEntry['status']
): WorkerThreadStatus | undefined {
  if (status === 'running') return 'running';
  if (status === 'success') return 'completed';
  if (status === 'error') return 'failed';
  return undefined;
}

/** Treat empty / structurally-empty tool bodies as absent. */
export function normalizeToolBody(value?: string): string | undefined {
  if (!value) return undefined;
  const trimmed = value.trim();
  if (trimmed.length === 0) return undefined;
  if (trimmed === '{}' || trimmed === '[]' || trimmed === 'null') return undefined;
  return value;
}

/**
 * Whether a timeline entry carries any unique body worth its own row — a
 * sub-agent's live activity, a returned result, a prompt/detail bubble, or a
 * structured failure. A row with none of these renders as a bare label + status
 * and is therefore indistinguishable from any sibling with the same title, so it
 * is safe to coalesce (see {@link coalesceTimelineEntries}). Mirrors the
 * `expandable` predicate in the row renderer so the two never disagree.
 */
export function entryHasUniqueBody(entry: ToolTimelineEntry): boolean {
  const formatted = formatTimelineEntry(entry);
  const detailContent = normalizeToolBody(formatted.detail) ?? normalizeToolBody(entry.argsBuffer);
  const resultContent = normalizeToolBody(entry.result);
  return (
    detailContent != null ||
    resultContent != null ||
    entry.subagent != null ||
    entry.failure != null
  );
}

/** A rendered timeline row: a representative entry plus how many identical,
 * body-less entries it stands in for (`count === 1` for an ordinary row). */
export interface CoalescedRow {
  entry: ToolTimelineEntry;
  count: number;
}

/**
 * Collapse runs of consecutive, identical, body-less rows into a single row
 * carrying an `×N` count. A retry loop (e.g. the orchestrator re-spawning the
 * integrations agent 25×, each surfacing the same "Checking your connected app"
 * label with no distinguishing detail) would otherwise flood the timeline with
 * indistinguishable nodes. Only truly interchangeable rows merge: same title,
 * same status, no unique body (result/detail/sub-agent/failure), and never the
 * live `running` row — so no information is lost, only duplication.
 */
export function coalesceTimelineEntries(entries: ToolTimelineEntry[]): CoalescedRow[] {
  const rows: CoalescedRow[] = [];
  for (const entry of entries) {
    const mergeable = entry.status !== 'running' && !entryHasUniqueBody(entry);
    const previous = rows[rows.length - 1];
    if (
      mergeable &&
      previous != null &&
      previous.entry.status === entry.status &&
      !entryHasUniqueBody(previous.entry) &&
      previous.entry.status !== 'running' &&
      formatTimelineEntry(previous.entry).title === formatTimelineEntry(entry).title
    ) {
      previous.count += 1;
      continue;
    }
    rows.push({ entry, count: 1 });
  }
  return rows;
}

/** Compact "×N" badge appended to a coalesced row's label. */
export function RepeatCount({ count }: { count: number }) {
  if (count <= 1) return null;
  return (
    <Badge className="shrink-0 rounded-full text-[10px]" data-testid="timeline-repeat-count">
      ×{count}
    </Badge>
  );
}

/**
 * Small "spark" glyph used as each agent's node on the timeline rail —
 * mirrors the Figma "Intelligence" icon. Inherits `currentColor` so the
 * caller controls its tone (muted while running, solid when done).
 */
export function AgentSparkIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 12 12"
      width="12"
      height="12"
      aria-hidden
      className={className}
      focusable="false">
      <path
        d="M6 0.4 L7.25 4.75 L11.6 6 L7.25 7.25 L6 11.6 L4.75 7.25 L0.4 6 L4.75 4.75 Z"
        fill="currentColor"
      />
    </svg>
  );
}

/**
 * Map a timeline row's lifecycle status to the agent-name text treatment.
 *
 * The Figma "Agentic task insights" design conveys per-agent progress
 * through the *name text* rather than a progress bar: an in-flight agent
 * pulses in a muted tone, a finished agent reads solid/full-strength, and
 * a failed agent is tinted with the coral error token. (Per product
 * direction — no numeric progress signal exists from the core, so we never
 * fabricate one.)
 */
export function agentNameTone(status: ToolTimelineEntryStatus | undefined): string {
  switch (status) {
    case 'success':
      return 'text-content-secondary dark:text-content';
    case 'error':
      return 'text-coral-600 dark:text-coral-300';
    case 'awaiting_user':
      return 'animate-pulse text-amber-600 dark:text-amber-300';
    case 'cancelled':
      return 'text-content-faint';
    default:
      return 'animate-pulse text-content-faint';
  }
}

/**
 * One row on the agent-insights timeline rail: a left column carrying the
 * spark node icon plus the vertical connector that threads consecutive
 * agents together, and an indented content column for the row body.
 */
export function AgentTimelineRow({
  isFirst = false,
  isLast = false,
  icon,
  iconClassName,
  children,
}: {
  isFirst?: boolean;
  isLast?: boolean;
  icon?: ReactNode;
  iconClassName?: string;
  children: ReactNode;
}) {
  return (
    <div className="relative flex gap-2.5" data-testid="agent-timeline-row">
      <div className="relative flex w-3 shrink-0 justify-center">
        {!isFirst ? (
          <span
            aria-hidden
            className="absolute top-0 left-1/2 h-[9px] w-px -translate-x-1/2 bg-surface-strong"
          />
        ) : null}
        {!isLast ? (
          <span
            aria-hidden
            className="absolute top-[9px] bottom-0 left-1/2 w-px -translate-x-1/2 bg-surface-strong"
          />
        ) : null}
        <span className="relative z-10 mt-0.5 flex h-3 w-3 items-center justify-center bg-[#f6f6f6] dark:bg-surface-canvas">
          {icon ?? <AgentSparkIcon className={iconClassName ?? 'text-content-faint'} />}
        </span>
      </div>
      <div className="min-w-0 flex-1 pb-2">{children}</div>
    </div>
  );
}
