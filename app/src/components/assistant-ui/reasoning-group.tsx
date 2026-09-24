'use client';

/**
 * The thread's reasoning block: a run of consecutive reasoning parts rendered
 * as one static `ReasoningPanel` ("Thinking… 4s" while it streams, then
 * "Thought for 12s" with the titled steps behind the disclosure).
 *
 * Timing rides on each part as `providerMetadata.openhuman.{startedAt,endedAt}`
 * (see `reasoningPart` in `providers/assistantUiMessages.ts`).
 */
import { useAuiState, useScrollLock } from '@assistant-ui/react';
import { memo, useMemo, useRef } from 'react';

import { ReasoningTrace } from './elements/reasoning-trace';
import type { ReasoningTiming } from './elements/reasoningSteps';

const ANIMATION_DURATION = 200;

/** Namespace of this app's entry in a part's `providerMetadata`. */
export const REASONING_TIMING_PROVIDER = 'openhuman';

type PartLike = {
  readonly type: string;
  readonly text?: string;
  readonly providerMetadata?: Readonly<Record<string, unknown>>;
};

/** Read the `{startedAt, endedAt}` a reasoning part carries, if any. */
export function reasoningTimingOf(part: PartLike | undefined): ReasoningTiming | undefined {
  const meta = part?.providerMetadata?.[REASONING_TIMING_PROVIDER];
  if (!meta || typeof meta !== 'object') return undefined;
  const { startedAt, endedAt } = meta as { startedAt?: unknown; endedAt?: unknown };
  const timing: ReasoningTiming = {};
  if (typeof startedAt === 'number') timing.startedAt = startedAt;
  if (typeof endedAt === 'number') timing.endedAt = endedAt;
  return timing.startedAt === undefined && timing.endedAt === undefined ? undefined : timing;
}

/** Stable-identity extraction so the trace only re-parses when a part changes. */
function useGroupReasoning(indices: readonly number[]) {
  const parts = useAuiState(s => s.message.parts) as readonly PartLike[];
  const cache = useRef<{ texts: string[]; timings: (ReasoningTiming | undefined)[] } | null>(null);

  return useMemo(() => {
    const texts: string[] = [];
    const timings: (ReasoningTiming | undefined)[] = [];
    for (const index of indices) {
      const part = parts[index];
      if (!part || part.type !== 'reasoning') continue;
      texts.push(part.text ?? '');
      timings.push(reasoningTimingOf(part));
    }
    const prev = cache.current;
    const same =
      prev !== null &&
      prev.texts.length === texts.length &&
      prev.texts.every((t, i) => t === texts[i]) &&
      prev.timings.every(
        (t, i) => t?.startedAt === timings[i]?.startedAt && t?.endedAt === timings[i]?.endedAt
      );
    if (same && prev) return prev;
    cache.current = { texts, timings };
    return cache.current;
  }, [parts, indices]);
}

export const OpenHumanReasoningGroup = memo(
  ({ indices, running }: { indices: readonly number[]; running: boolean }) => {
    const rootRef = useRef<HTMLDivElement>(null);
    const lockScroll = useScrollLock(rootRef, ANIMATION_DURATION);
    const { texts, timings } = useGroupReasoning(indices);

    return (
      <div ref={rootRef} data-slot="aui_reasoning-group">
        <ReasoningTrace
          texts={texts}
          timings={timings}
          streaming={running}
          onAnimationStart={lockScroll}
          data-testid="reasoning-panel"
        />
      </div>
    );
  }
);
OpenHumanReasoningGroup.displayName = 'OpenHumanReasoningGroup';
