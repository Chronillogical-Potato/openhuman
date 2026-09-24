/**
 * One web source the agent visited, as a link row.
 *
 * Shared by the two surfaces that list sources — the process rail
 * (`AgentProcessSourcePanel`) and the inline row under a settled answer
 * (`aui/ChatSources`). Deliberately shared rather than duplicated: this is the
 * component that renders a model-supplied URL into an `<a href>`, and the URL
 * is a raw tool-call argument, so it is prompt-injection-influenceable. The
 * scheme filtering happens upstream in `extractAgentSources`
 * (`utils/toolTimelineFormatting.ts`, `isHttpUrl`), and keeping one row
 * component is what stops a second surface from growing its own link markup
 * that forgets to come through that extractor.
 *
 * Layout stays with each caller — the rail's list and the inline disclosure
 * want different density — but the anchor does not.
 */
import { Source } from '../../../components/ai-elements';
import type { AgentSource } from '../../../utils/toolTimelineFormatting';

/** Compact globe glyph for a source row. Inherits `currentColor`. */
export function GlobeIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 12 12"
      width="12"
      height="12"
      aria-hidden
      className={className}
      focusable="false">
      <circle cx="6" cy="6" r="5" fill="none" stroke="currentColor" strokeWidth="1" />
      <path
        d="M1 6h10M6 1c1.8 1.4 1.8 8.6 0 10M6 1c-1.8 1.4-1.8 8.6 0 10"
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
      />
    </svg>
  );
}

export function AgentSourceRow({ source }: { source: AgentSource }) {
  return (
    <li>
      <Source
        href={source.url}
        rel="noreferrer noopener"
        className="flex items-center justify-between gap-3 rounded-md px-1.5 py-1 text-[11px] text-content-secondary hover:bg-surface-hover"
        data-testid="agent-source-row">
        <span className="flex min-w-0 items-center gap-1.5">
          <GlobeIcon className="shrink-0 text-content-faint" />
          <span className="truncate text-content-secondary">{source.title}</span>
        </span>
        <span className="shrink-0 truncate text-content-faint">{source.url}</span>
      </Source>
    </li>
  );
}

export default AgentSourceRow;
