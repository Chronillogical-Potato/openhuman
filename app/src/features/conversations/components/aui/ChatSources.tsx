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
import { Sources } from '../../../../components/assistant-ui/elements/sources.aui';
import type { SourceItemPart } from '../../../../components/assistant-ui/thread';
import { useT } from '../../../../lib/i18n/I18nContext';

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
          <Sources key={source.id} sourceType="url" url={source.url} title={source.title} />
        ) : (
          <Sources
            key={source.id}
            sourceType="document"
            title={source.title ?? t('conversations.agentTaskInsights.memoryCitationFallbackTitle')}
          />
        )
      )}
    </section>
  );
}

export default ChatSources;
