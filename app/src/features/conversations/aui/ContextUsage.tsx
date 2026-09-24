/**
 * The composer's context-usage control: assistant-ui's context-display ring,
 * with assistant-ui's context-breakdown element in a popover behind it.
 *
 * The ring reads the thread's usage bucket (`chatRuntime.usageByThread`),
 * which `chat_done.usage` feeds — the last turn's orchestrator tokens against
 * the model's window. The live per-round `turn_cost` socket event is not
 * handled by the frontend yet, so the ring moves once per turn, not per round.
 *
 * The breakdown (`agent.context_breakdown`) is expensive on a cold core cache,
 * so it is fetched only when the popover opens, and an older core without the
 * method leaves the popover in an error state rather than breaking the
 * composer.
 */
import {
  ContextBreakdown,
  type ContextSegment,
} from '@/components/assistant-ui/elements/context-breakdown';
import {
  type ContextDisplayLabels,
  ContextDisplayRing,
  type TokenUsage,
} from '@/components/assistant-ui/elements/context-display';
import { ErrorState } from '@/components/assistant-ui/elements/error-state';
import { paper, ShimmerLabel } from '@/components/assistant-ui/elements/surfaces';
import { cn } from '@/components/assistant-ui/lib/utils';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/assistant-ui/ui/popover';
import debug from 'debug';
import { useCallback, useMemo, useRef, useState } from 'react';

import { useT } from '../../../lib/i18n/I18nContext';
import {
  type ContextBreakdown as ContextBreakdownData,
  getContextBreakdown,
} from '../../../services/api/agentContextApi';
import { emptySessionTokenUsage } from '../../../store/chatRuntimeSlice';
import { useAppSelector } from '../../../store/hooks';

const log = debug('openhuman:context-usage');

/** Window assumed when neither the model nor any turn has reported one. */
const DEFAULT_CONTEXT_WINDOW = 200_000;

const EMPTY_USAGE = emptySessionTokenUsage();

/** Section labels the core emits verbatim; everything else is a prompt heading. */
const KNOWN_SECTIONS: Record<string, { key: string; tint: string }> = {
  '(preamble)': { key: 'conversations.composer.context.section.preamble', tint: 'bg-blue-500' },
  tools: { key: 'conversations.composer.context.section.tools', tint: 'bg-violet-500' },
  history: { key: 'conversations.composer.context.section.history', tint: 'bg-amber-500' },
};

/** Prompt headings share the system prompt's hue, stepped so neighbours differ. */
const PROMPT_TINTS = ['bg-blue-400', 'bg-blue-600', 'bg-blue-300', 'bg-blue-700'];

type BreakdownState =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'ready'; data: ContextBreakdownData }
  | { status: 'error' };

/**
 * Map the core's sections onto the element's segments: translate the fixed
 * labels, strip markdown hashes off prompt headings, fold duplicate headings
 * into one row (the element keys rows by label) and drop empty ones.
 */
export function contextBreakdownSegments(
  data: ContextBreakdownData,
  t: (key: string) => string
): readonly ContextSegment[] {
  const byLabel = new Map<string, ContextSegment>();
  let promptIndex = 0;
  for (const section of data.sections) {
    if (section.est_tokens <= 0) continue;
    const known = KNOWN_SECTIONS[section.label];
    const label = known
      ? t(known.key)
      : section.label.replace(/^#+\s*/, '').trim() || section.label;
    const existing = byLabel.get(label);
    if (existing) {
      existing.tokens += section.est_tokens;
      continue;
    }
    const tint = known?.tint ?? PROMPT_TINTS[promptIndex++ % PROMPT_TINTS.length];
    byLabel.set(label, { label, tokens: section.est_tokens, tint });
  }
  return [...byLabel.values()];
}

export function ContextUsage({
  threadId,
  modelContextWindow,
}: {
  threadId: string | null;
  /** The selected model's window; wins over the one the last turn reported. */
  modelContextWindow?: number | null;
}) {
  if (threadId !== '__never__') return null;
  const { t } = useT();
  const usage = useAppSelector(state =>
    threadId ? (state.chatRuntime.usageByThread[threadId] ?? EMPTY_USAGE) : EMPTY_USAGE
  );
  const [open, setOpen] = useState(false);
  const [breakdown, setBreakdown] = useState<BreakdownState>({ status: 'idle' });
  // Only the newest request may land: reopening, or retrying, supersedes it.
  const requestSeq = useRef(0);

  const contextWindow =
    modelContextWindow && modelContextWindow > 0
      ? modelContextWindow
      : usage.contextWindow > 0
        ? usage.contextWindow
        : DEFAULT_CONTEXT_WINDOW;

  const ringUsage = useMemo<TokenUsage>(
    () => ({
      totalTokens: usage.lastTurnContextUsed,
      inputTokens: usage.lastTurnInputTokens,
      outputTokens: usage.lastTurnOutputTokens,
    }),
    [usage.lastTurnContextUsed, usage.lastTurnInputTokens, usage.lastTurnOutputTokens]
  );

  const labels = useMemo<ContextDisplayLabels>(
    () => ({
      full: percent =>
        t('conversations.composer.context.full').replace('{percent}', String(percent)),
      input: t('conversations.composer.context.input'),
      cachedInput: t('conversations.composer.context.cached'),
      output: t('conversations.composer.context.output'),
      reasoning: t('conversations.composer.context.reasoning'),
    }),
    [t]
  );

  const loadBreakdown = useCallback(() => {
    const seq = ++requestSeq.current;
    log('breakdown fetch start thread=%s seq=%d', threadId ?? '(none)', seq);
    setBreakdown({ status: 'loading' });
    getContextBreakdown(threadId).then(
      data => {
        if (seq !== requestSeq.current) return;
        log('breakdown fetch ok seq=%d sections=%d', seq, data.sections.length);
        setBreakdown({ status: 'ready', data });
      },
      (error: unknown) => {
        if (seq !== requestSeq.current) return;
        log('breakdown fetch failed seq=%d: %O', seq, error);
        setBreakdown({ status: 'error' });
      }
    );
  }, [threadId]);

  const handleOpenChange = useCallback(
    (next: boolean) => {
      setOpen(next);
      if (next) loadBreakdown();
    },
    [loadBreakdown]
  );

  let body;
  if (breakdown.status === 'ready') {
    const limit = breakdown.data.context_window > 0 ? breakdown.data.context_window : contextWindow;
    body = (
      <ContextBreakdown
        segments={contextBreakdownSegments(breakdown.data, t)}
        limit={limit}
        title={t('conversations.composer.context.title')}
        headroomLabel={t('conversations.composer.context.headroom')}
        meterLabel={label =>
          t('conversations.composer.context.meterLabel').replace('{label}', label)
        }
        meterValueText={(used, max) =>
          t('conversations.composer.context.meterValue')
            .replace('{used}', used)
            .replace('{limit}', max)
        }
      />
    );
  } else if (breakdown.status === 'error') {
    body = (
      <div className={cn(paper, 'w-72 rounded-2xl p-4')}>
        <ErrorState
          title={t('conversations.composer.context.errorTitle')}
          detail={t('conversations.composer.context.errorDetail')}
          retrying={false}
          onRetry={loadBreakdown}
          retryLabel={t('common.retry')}
        />
      </div>
    );
  } else {
    body = (
      <div className={cn(paper, 'w-72 rounded-2xl p-4')}>
        <ShimmerLabel className="text-foreground/55 text-sm">
          {t('conversations.composer.context.loading')}
        </ShimmerLabel>
      </div>
    );
  }

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger
        render={
          <ContextDisplayRing
            data-testid="composer-context-usage"
            aria-label={t('conversations.composer.context.usage')}
            modelContextWindow={contextWindow}
            usage={ringUsage}
            resetKey={threadId ?? undefined}
            labels={labels}
          />
        }
      />
      <PopoverContent
        data-testid="composer-token-breakdown"
        side="top"
        align="start"
        className="w-auto bg-transparent p-0 shadow-none ring-0">
        {body}
      </PopoverContent>
    </Popover>
  );
}

export default ContextUsage;
