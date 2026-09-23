/**
 * The web sources a settled turn visited, listed inline under its answer.
 *
 * ## Why inline, when the rail already lists them
 *
 * The process rail (`AgentProcessSourcePanel`) has shown sources for a while,
 * behind the `TurnFooter` click. This is not a second copy of something already
 * on screen: on a settled turn the URL is not visible anywhere without a click.
 * `web_fetch` implements no `display_detail`, so its tool card's collapsed
 * trigger carries no URL chip (`AssistantUiToolCall` renders that chip only when
 * `detail` is present), and the arguments live in the card's body inside a
 * `Collapsible` whose `defaultOpen` is `running` — i.e. closed once the turn
 * finishes. So "which pages did it read?" currently costs either one expand per
 * fetch card or a trip to the rail.
 *
 * ## Why it reuses `extractAgentSources`
 *
 * Sources are not a model-declared message part — nothing in this app emits an
 * assistant-ui `source` part. They are derived client-side from the `url`
 * argument of the fetch/browse tools (`URL_SOURCE_TOOLS`). That argument is raw
 * model output and therefore prompt-injection-influenceable, which is why
 * `extractAgentSources` admits only `http(s)` URLs — a `javascript:` or `file:`
 * value must never reach an `<a href>`. Deriving sources here through any other
 * path would mean re-implementing that filter, so this calls the one extractor
 * and renders the one shared row component.
 *
 * Collapsed by default, unlike the rail's copy: the answer stays the top of the
 * turn. Same reasoning that put the process behind the footer to begin with.
 */
import { type AssistantState, useAuiState } from '@assistant-ui/react';

import { Sources, SourcesContent, SourcesTrigger } from '../../../../components/ai-elements';
import { useT } from '../../../../lib/i18n/I18nContext';
import { extractAgentSources } from '../../../../utils/toolTimelineFormatting';
import { AgentSourceRow } from '../AgentSourceRow';
import { readProcessTrail } from './turnProcessTrail';

const selectMessageMetadata = (state: AssistantState) => state.message.metadata;

export function TurnSources() {
  const { t } = useT();
  const trail = readProcessTrail(useAuiState(selectMessageMetadata));
  // `readProcessTrail` now validates `timeline` is an array, so this cannot be
  // an unchecked read; a message with no trail at all yields `null` above.
  const sources = trail ? extractAgentSources([...trail.timeline]) : [];

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
              <AgentSourceRow key={source.id} source={source} />
            ))}
          </ul>
        </SourcesContent>
      </section>
    </Sources>
  );
}

export default TurnSources;
