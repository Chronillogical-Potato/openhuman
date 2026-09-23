/**
 * The web sources a turn visited, as one collapsed disclosure under its answer.
 *
 * The sources arrive as assistant-ui `source` parts, emitted by `assistantParts`
 * (`providers/assistantUiMessages.ts`) through `extractAgentSources`, which is
 * the one place a model-supplied URL is admitted (http(s) only) — the `url` is a
 * raw tool-call argument and so prompt-injection-influenceable. `Thread` groups
 * the run of source parts and hands them here through its `SourceGroup` slot.
 *
 * Collapsed by default: the answer stays the top of the turn.
 */
import type { SourceUrlPart } from '../../../../components/assistant-ui/thread';
import { Sources, SourcesContent, SourcesTrigger } from '../../../../components/ai-elements';
import { useT } from '../../../../lib/i18n/I18nContext';
import { AgentSourceRow } from '../AgentSourceRow';

export function ChatSources({ sources }: { sources: readonly SourceUrlPart[] }) {
  const { t } = useT();
  if (sources.length === 0) return null;

  return (
    <Sources asChild className="mb-0 text-content-muted">
      <section data-testid="turn-sources">
        <SourcesTrigger
          count={sources.length}
          className="text-content-muted hover:text-content-secondary text-xs transition-colors">
          {t('conversations.agentTaskInsights.sourcesHeading')} ({sources.length})
        </SourcesTrigger>
        <SourcesContent className="mt-1 w-full gap-0">
          <ul className="space-y-0.5">
            {sources.map(source => (
              <AgentSourceRow
                key={source.id}
                source={{ id: source.id, url: source.url, title: source.title ?? source.url }}
              />
            ))}
          </ul>
        </SourcesContent>
      </section>
    </Sources>
  );
}

export default ChatSources;
