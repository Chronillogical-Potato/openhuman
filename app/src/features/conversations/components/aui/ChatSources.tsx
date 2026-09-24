/**
 * The sources a turn drew on, as one row of source badges under its answer:
 * `url` sources (web fetch/search) and `document` sources (memory citations).
 *
 * Sources arrive as assistant-ui `source` parts, emitted by `assistantParts`
 * (`providers/assistantUiMessages.ts`) through `extractAgentSources` for
 * `url` (the one place a model-supplied URL is admitted, http(s) only — a raw
 * tool-call argument, so prompt-injection-influenceable) and directly from
 * the turn's `citations` (memory retrieval) for `document`. `Thread` groups
 * the run of source parts and hands them here through its `SourceGroup` slot.
 *
 * Renders through the vendored `sources.aui` element's per-part `Sources`
 * component (a `SourceMessagePartComponent`) rather than the old collapsible
 * `components/ai-elements/Sources.tsx` disclosure — every source shows as a
 * badge/link inline, nothing hidden behind a click.
 */
import { Badge } from '../../../../components/assistant-ui/badge';
import {
  DocumentSourceIcon,
  Source,
  SourceIcon,
  SourceTitle,
} from '../../../../components/assistant-ui/elements/sources.aui';
import type { SourceItemPart } from '../../../../components/assistant-ui/thread';
import { useT } from '../../../../lib/i18n/I18nContext';

/**
 * Composes the vendored `sources.aui` primitives (`Source`/`SourceIcon`/
 * `SourceTitle`/`DocumentSourceIcon`/`Badge`) directly rather than calling its
 * `Sources` message-part component: that component's prop type is the full
 * assistant-ui `SourceMessagePartProps` (part `status`, `mediaType`, ...),
 * which this app's `SourceItemPart` (derived from `extractAgentSources` /
 * memory citations, not a live message-part subscription) does not carry.
 */
export function ChatSources({ sources }: { sources: readonly SourceItemPart[] }) {
  const { t } = useT();
  if (sources.length === 0) return null;

  return (
    <section
      data-testid="turn-sources"
      aria-label={t('conversations.agentTaskInsights.sourcesHeading')}
      className="mt-1 flex flex-wrap items-center gap-1.5">
      {sources.map(source =>
        source.sourceType === 'url' ? (
          <Source key={source.id} href={source.url} data-testid="agent-source-row">
            <SourceIcon url={source.url} />
            <SourceTitle>{source.title || source.url}</SourceTitle>
          </Source>
        ) : (
          <Badge key={source.id} variant="secondary" data-testid="agent-memory-source-row">
            <span className="inline-flex items-center gap-1.5">
              <DocumentSourceIcon />
              <SourceTitle>
                {source.title ?? t('conversations.agentTaskInsights.memoryCitationFallbackTitle')}
              </SourceTitle>
            </span>
          </Badge>
        )
      )}
    </section>
  );
}

export default ChatSources;
