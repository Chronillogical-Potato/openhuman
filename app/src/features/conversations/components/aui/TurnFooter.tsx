import { type AssistantState, useAuiState } from '@assistant-ui/react';

import { useT } from '../../../../lib/i18n/I18nContext';
import { useTurnFooterHost } from './turnFooterHost';
import { readProcessTrail } from './turnProcessTrail';

const selectMessageMetadata = (state: AssistantState) => state.message.metadata;

/**
 * The settled turn's one-line process footer — `8 steps · 2 tools` — and the
 * single door to the detail behind it.
 *
 * This is the other half of moving reasoning, narration and read-only tool
 * detail off the main surface (`assistantParts`): the transcript keeps the
 * answer, and everything explaining how the agent got there is one click away
 * in the process rail rather than stacked above the answer.
 *
 * Deliberately quiet. It is process, not answer, so it takes the same
 * treatment the rail already gives its own process text — `text-content-muted`
 * at 12px — while the answer above it keeps full-contrast `text-foreground` at
 * its normal size. Same meaning, same look, on both surfaces.
 *
 * Renders nothing when the turn recorded no process (a plain answer needs no
 * door) or when no host is mounted to open the rail.
 */
export function TurnFooter() {
  const { t } = useT();
  const host = useTurnFooterHost();
  const trail = readProcessTrail(useAuiState(selectMessageMetadata));

  if (!host || !trail || trail.steps === 0) return null;

  // Existing, fully-translated keys rather than new ones: `I18nContext`'s
  // English fallback is real, but `i18n/__tests__/coverage.test.ts` requires
  // every locale to define every English key, so a new key is 14 translations
  // this change is in no position to write. The cost is that
  // `conversations.backgroundTasks.steps` has no singular form, so a one-step
  // turn reads "1 steps". Worth a proper key next time someone touches i18n.
  const stepLabel = t('conversations.backgroundTasks.steps').replace(
    '{count}',
    String(trail.steps)
  );
  const toolLabel =
    trail.tools > 0
      ? t(
          trail.tools === 1
            ? 'intelligence.agents.toolCountOne'
            : 'intelligence.agents.toolCountOther'
        ).replace('{count}', String(trail.tools))
      : null;

  return (
    <button
      type="button"
      data-testid="turn-process-footer"
      data-analytics-id="chat-turn-process-open"
      title={t('conversations.agentTaskInsights.viewProcessSource')}
      onClick={() => host.open(trail)}
      className="text-content-muted hover:text-content-secondary -ms-1 rounded px-1 text-xs transition-colors">
      {toolLabel ? `${stepLabel} · ${toolLabel}` : stepLabel}
    </button>
  );
}

export default TurnFooter;
