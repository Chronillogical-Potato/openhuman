'use client';

/**
 * The static reasoning element, vendored from assistant-ui's
 * `elements-reasoning-panel` registry item
 * (https://r.assistant-ui.com/elements-reasoning-panel.json). The markup and
 * classes are upstream's; the local additions are all behavioural:
 *
 * - `collapsible={false}` renders the same header and step list with no
 *   disclosure, for surfaces where reasoning stays inline and visible.
 * - `open` is optional. Uncontrolled, it follows the runtime reasoning
 *   element's rule: open while streaming, back to `defaultOpen` once settled,
 *   and a manual toggle wins from then on.
 * - `liveLabel` replaces the hard-coded "Thinking" (the newest step title
 *   reads best, Codex style) and localizes it.
 * - Step bodies render through the registry's own `MarkdownText` (via
 *   assistant-ui's `TextMessagePartProvider`) instead of a plain `<p>`.
 * - A streaming list stays pinned to its newest step inside a bounded
 *   scroll region (the runtime element's `ReasoningText` behaviour).
 */
import { cn } from '@/components/assistant-ui/lib/utils';
import { MarkdownText } from '@/components/assistant-ui/markdown-text';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/assistant-ui/ui/collapsible';
import { TextMessagePartProvider } from '@assistant-ui/react';
import { ChevronDownIcon } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';

import { take } from '../utils/range';
import type { ReasoningStep } from './reasoningSteps';
import { collapsePanel, mono, ShimmerLabel, SwapLabel } from './surfaces';

export type { ReasoningStep } from './reasoningSteps';

export interface ReasoningPanelProps {
  steps: readonly ReasoningStep[];
  /** How many steps to reveal; defaults to all of them. */
  visibleSteps?: number;
  streaming: boolean;
  /** Label once streaming ends, e.g. "Thought for 12s". */
  restingLabel: string;
  /** Label while streaming (upstream hard-codes "Thinking"). */
  liveLabel: string;
  /** Elapsed badge beside the live label, e.g. "4s". */
  elapsed?: string;
  /** Disclosure (default) or an always-visible trace. */
  collapsible?: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  defaultOpen?: boolean;
  /** Called right before the disclosure animates (scroll-lock hook point). */
  onAnimationStart?: () => void;
  className?: string;
  'data-testid'?: string;
}

/**
 * Keeps a bounded scroll region pinned to its newest content while `active`;
 * following pauses while the reader is scrolled up. Ported from the runtime
 * reasoning element's `ReasoningText`.
 */
function usePinnedScroll(active: boolean) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLOListElement>(null);

  useEffect(() => {
    if (!active) return;
    const scrollEl = scrollRef.current;
    const contentEl = contentRef.current;
    if (!scrollEl || !contentEl) return;

    let pinned = true;
    let lastScrollTop = scrollEl.scrollTop;
    let lastScrollHeight = scrollEl.scrollHeight;
    const isAtBottom = () =>
      Math.abs(scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight) <= 1 ||
      scrollEl.scrollHeight <= scrollEl.clientHeight;
    const pin = () => {
      if (pinned) scrollEl.scrollTop = scrollEl.scrollHeight;
    };
    // A pin's own scroll event can land after new content grew the height and
    // read as "not at bottom"; only an upward move at unchanged height is the
    // reader's intent.
    const onScroll = () => {
      if (isAtBottom()) pinned = true;
      else if (scrollEl.scrollTop < lastScrollTop && scrollEl.scrollHeight === lastScrollHeight) {
        pinned = false;
      }
      lastScrollTop = scrollEl.scrollTop;
      lastScrollHeight = scrollEl.scrollHeight;
    };

    pin();
    scrollEl.addEventListener('scroll', onScroll);
    const observer = new ResizeObserver(pin);
    observer.observe(contentEl);
    return () => {
      scrollEl.removeEventListener('scroll', onScroll);
      observer.disconnect();
    };
  }, [active]);

  return { scrollRef, contentRef };
}

function PanelLabel({
  streaming,
  liveLabel,
  restingLabel,
  elapsed,
}: Pick<ReasoningPanelProps, 'streaming' | 'liveLabel' | 'restingLabel' | 'elapsed'>) {
  return (
    <SwapLabel active={streaming ? 0 : 1} className="text-start">
      <>
        <ShimmerLabel
          active={streaming}
          data-slot="reasoning-panel-live-label"
          className="relative inline-block leading-none">
          {liveLabel}
        </ShimmerLabel>
        {elapsed !== undefined && (
          <span
            data-slot="reasoning-panel-elapsed"
            className={cn(mono, 'text-foreground/30 tabular-nums')}>
            {elapsed}
          </span>
        )}
      </>
      <span data-slot="reasoning-panel-resting-label">{restingLabel}</span>
    </SwapLabel>
  );
}

function StepList({
  steps,
  streaming,
  bounded,
}: {
  steps: readonly ReasoningStep[];
  streaming: boolean;
  bounded: boolean;
}) {
  const { scrollRef, contentRef } = usePinnedScroll(streaming && bounded);
  return (
    <div
      ref={scrollRef}
      data-slot="reasoning-panel-scroll"
      className={cn(bounded && 'max-h-80 overflow-y-auto')}>
      <ol ref={contentRef} className="flex flex-col gap-4 pt-3 pb-1">
        {steps.map((step, i) => {
          const active = streaming && i === steps.length - 1;
          return (
            <li
              // Titles can repeat across rounds, so the index is the key.
              key={i}
              data-slot="reasoning-panel-step"
              data-active={active ? '' : undefined}
              className="fade-in slide-in-from-bottom-1 animate-in fill-mode-both flex gap-3 duration-300">
              <span
                aria-hidden
                className={cn(
                  'mt-[7px] size-[5px] shrink-0 rounded-full transition-colors duration-300',
                  active ? 'animate-pulse bg-blue-500 dark:bg-blue-400' : 'bg-foreground/20'
                )}
              />
              <span className="flex min-w-0 flex-1 flex-col">
                <p
                  data-slot="reasoning-step-title"
                  className="text-foreground/90 text-[13.5px] font-medium">
                  {step.title}
                </p>
                {step.body ? (
                  <div
                    data-slot="reasoning-step-body"
                    className="text-foreground/50 mt-0.5 text-[13px] leading-relaxed break-words">
                    <TextMessagePartProvider text={step.body} isRunning={active}>
                      <MarkdownText />
                    </TextMessagePartProvider>
                  </div>
                ) : null}
              </span>
            </li>
          );
        })}
      </ol>
    </div>
  );
}

export function ReasoningPanel({
  steps,
  visibleSteps,
  streaming,
  restingLabel,
  liveLabel,
  elapsed,
  collapsible = true,
  open: controlledOpen,
  onOpenChange,
  defaultOpen = false,
  onAnimationStart,
  className,
  'data-testid': testId,
}: ReasoningPanelProps) {
  const shown = take(steps, visibleSteps ?? steps.length);
  const [userOpen, setUserOpen] = useState<boolean | null>(null);
  const isControlled = controlledOpen !== undefined;
  const isOpen = isControlled ? controlledOpen : (userOpen ?? (streaming || defaultOpen));

  // Streaming → settled collapses the panel (unless the reader took over),
  // an animation the host may need to scroll-lock.
  const prevStreaming = useRef(streaming);
  useEffect(() => {
    if (prevStreaming.current === streaming) return;
    prevStreaming.current = streaming;
    if (!isControlled && userOpen === null && !defaultOpen) onAnimationStart?.();
  }, [streaming, isControlled, userOpen, defaultOpen, onAnimationStart]);

  const handleOpenChange = useCallback(
    (next: boolean) => {
      onAnimationStart?.();
      if (!isControlled) setUserOpen(next);
      onOpenChange?.(next);
    },
    [onAnimationStart, isControlled, onOpenChange]
  );

  const label = (
    <PanelLabel
      streaming={streaming}
      liveLabel={liveLabel}
      restingLabel={restingLabel}
      elapsed={elapsed}
    />
  );

  if (!collapsible) {
    return (
      <div
        data-slot="reasoning-panel"
        data-variant="static"
        data-testid={testId}
        aria-busy={streaming || undefined}
        className={cn('w-full', className)}>
        <div className="text-foreground/55 flex items-center gap-1.5 py-1 text-[13.5px]">
          {label}
        </div>
        {shown.length > 0 && <StepList steps={shown} streaming={streaming} bounded={streaming} />}
      </div>
    );
  }

  return (
    <Collapsible
      data-slot="reasoning-panel"
      data-variant="collapsible"
      data-testid={testId}
      open={isOpen}
      onOpenChange={handleOpenChange}
      className={cn('w-full', className)}>
      <CollapsibleTrigger
        disabled={shown.length === 0}
        className="group/trigger text-foreground/55 hover:text-foreground/90 flex items-center gap-1.5 py-1 text-[13.5px] transition-[color,scale] outline-none active:scale-[0.98] disabled:pointer-events-none">
        {label}
        {shown.length > 0 && (
          <ChevronDownIcon className="size-3.5 shrink-0 opacity-60 transition-transform duration-200 ease-[cubic-bezier(0.32,0.72,0,1)] group-data-[state=open]/trigger:rotate-180 motion-reduce:transition-none" />
        )}
      </CollapsibleTrigger>
      <CollapsibleContent
        aria-busy={streaming || undefined}
        className={cn(collapsePanel, 'outline-none')}>
        <StepList steps={shown} streaming={streaming} bounded />
      </CollapsibleContent>
    </Collapsible>
  );
}
