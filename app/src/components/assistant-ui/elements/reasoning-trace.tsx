'use client';

/**
 * `ReasoningTrace` is the one entry point every surface uses to show the
 * agent's reasoning: raw trace text (one string per model round) plus optional
 * timing in, the static `ReasoningPanel` out, with localized labels.
 *
 * - While streaming: the newest heading (or "Thinking") shimmers beside a
 *   ticking elapsed badge.
 * - Once settled: "Thought for 12s" — frozen from the recorded timestamps, not
 *   the ticking clock, so a reload shows the same number.
 */
import { useT } from '@/lib/i18n/I18nContext';
import { useEffect, useMemo, useState } from 'react';

import { ReasoningPanel } from './reasoning-panel';
import {
  formatElapsed,
  latestHeading,
  parseReasoningParts,
  reasoningSpan,
  type ReasoningTiming,
  thoughtForLabel,
} from './reasoningSteps';

/** Milliseconds since `startedAt`, re-rendered every second while `active`. */
export function useElapsedMs(startedAt: number | undefined, active: boolean): number | undefined {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active || startedAt === undefined) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [active, startedAt]);
  if (startedAt === undefined) return undefined;
  return Math.max(0, now - startedAt);
}

export interface ReasoningTraceProps {
  /** The trace, one entry per reasoning part / model round. */
  texts: readonly string[];
  /** Per-part timing aligned with `texts` (entries may be missing). */
  timings?: readonly (ReasoningTiming | undefined)[];
  streaming: boolean;
  collapsible?: boolean;
  defaultOpen?: boolean;
  onAnimationStart?: () => void;
  className?: string;
  'data-testid'?: string;
}

export function ReasoningTrace({
  texts,
  timings,
  streaming,
  collapsible = true,
  defaultOpen,
  onAnimationStart,
  className,
  'data-testid': testId,
}: ReasoningTraceProps) {
  const { t } = useT();
  const steps = useMemo(() => parseReasoningParts(texts), [texts]);
  const span = useMemo(() => reasoningSpan(timings ?? []), [timings]);
  const elapsedMs = useElapsedMs(span?.startedAt, streaming);

  const liveLabel = latestHeading(steps) ?? t('chat.reasoning.thinking');
  const settledMs = span && span.endedAt !== undefined ? span.endedAt - span.startedAt : undefined;
  const restingLabel = thoughtForLabel(settledMs, t);

  return (
    <ReasoningPanel
      steps={steps}
      streaming={streaming}
      liveLabel={liveLabel}
      restingLabel={restingLabel}
      elapsed={streaming && elapsedMs !== undefined ? formatElapsed(elapsedMs, t) : undefined}
      collapsible={collapsible}
      defaultOpen={defaultOpen}
      onAnimationStart={onAnimationStart}
      className={className}
      data-testid={testId}
    />
  );
}

/** One reasoning text (a single block) through {@link ReasoningTrace}. */
export function ReasoningTraceText({
  text,
  timing,
  ...rest
}: Omit<ReasoningTraceProps, 'texts' | 'timings'> & { text: string; timing?: ReasoningTiming }) {
  const texts = useMemo(() => [text], [text]);
  const timings = useMemo(() => [timing], [timing?.startedAt, timing?.endedAt]); // eslint-disable-line react-hooks/exhaustive-deps
  return <ReasoningTrace texts={texts} timings={timings} {...rest} />;
}
