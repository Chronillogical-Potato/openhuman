'use client';

/**
 * The static reasoning element, vendored from assistant-ui's
 * `elements-reasoning-panel` registry item and extended for this app.
 *
 * It is prop-driven (no runtime): an ordered list of titled steps along a
 * timeline, with a trigger that shimmers the live label and an elapsed badge
 * while the trace streams, then settles into a resting label such as
 * "Thought for 12s".
 *
 * Additions over upstream:
 * - `collapsible={false}` renders the same header and steps with no
 *   disclosure, for surfaces where reasoning stays inline and visible.
 * - Uncontrolled open state with the runtime element's rule: open while
 *   streaming, back to `defaultOpen` once settled, and a manual toggle wins
 *   from then on.
 * - `liveLabel` (Codex-style): the newest step title replaces the bare
 *   "Thinking" while streaming.
 * - Step bodies render markdown, and a long live trace stays pinned to its
 *   newest tokens inside a bounded scroll region.
 */
import { cn } from '@/components/assistant-ui/lib/utils';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/assistant-ui/ui/collapsible';
import { ChevronDownIcon } from 'lucide-react';
import { memo, useCallback, useEffect, useRef, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

import { take } from '../utils/range';
import type { ReasoningStep } from './reasoningSteps';
import { collapsePanel, mono, ShimmerLabel, SwapLabel } from './surfaces';

export type { ReasoningStep } from './reasoningSteps';

export interface ReasoningPanelProps {
  steps: readonly ReasoningStep[];
  /** How many steps to reveal; defaults to all of them. */
  visibleSteps?: number;
  streaming: boolean;
  /** Label shown once streaming ends, e.g. "Thought for 12s". */
  restingLabel: string;
  /** Label shown while streaming; the latest step title reads best. */
  liveLabel: string;
  /** Elapsed badge shown next to the live label, e.g. "4s". */
  elapsed?: string;
  /** Render as a disclosure (default) or as an always-visible trace. */
  collapsible?: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  defaultOpen?: boolean;
  /** Called right before the disclosure animates (scroll-lock hook point). */
  onAnimationStart?: () => void;
  className?: string;
  'data-testid'?: string;
}

const REMARK_PLUGINS = [remarkGfm];

const StepBody = memo(({ body }: { body: string }) => {
  return (
    <div
      data-slot="reasoning-step-body"
      className={cn(
        'text-foreground/55 mt-0.5 text-[13px] leading-relaxed break-words',
        '[&_p]:my-1 [&_p:first-child]:mt-0 [&_p:last-child]:mb-0',
        '[&_ul]:my-1 [&_ul]:list-disc [&_ul]:ps-4 [&_ol]:my-1 [&_ol]:list-decimal [&_ol]:ps-4',
        '[&_code]:bg-foreground/[0.06] [&_code]:rounded [&_code]:px-1 [&_code]:font-mono [&_code]:text-[12px]',
        '[&_pre]:bg-foreground/[0.04] [&_pre]:my-1.5 [&_pre]:overflow-x-auto [&_pre]:rounded-md [&_pre]:p-2',
        '[&_strong]:text-foreground/75 [&_a]:underline'
      )}>
      <ReactMarkdown remarkPlugins={REMARK_PLUGINS}>{body}</ReactMarkdown>
    </div>
  );
});
StepBody.displayName = 'ReasoningStepBody';

/**
 * Keeps a bounded scroll region pinned to its newest content while `active`,
 * and stops following once the reader scrolls up (resumes at the bottom).
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
    <SwapLabel active={streaming ? 0 : 1} className="min-w-0 text-start">
      <>
        <ShimmerLabel
          active={streaming}
          data-slot="reasoning-panel-live-label"
          className="relative inline-block max-w-[22rem] truncate py-0.5 leading-none">
          {liveLabel}
        </ShimmerLabel>
        {elapsed !== undefined && (
          <span
            data-slot="reasoning-panel-elapsed"
            className={cn(mono, 'text-foreground/35 tabular-nums')}>
            {elapsed}
          </span>
        )}
      </>
      <span data-slot="reasoning-panel-resting-label" className="py-0.5">
        {restingLabel}
      </span>
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
      className={cn(bounded && 'max-h-80 overflow-y-auto overscroll-contain')}>
      <ol ref={contentRef} data-slot="reasoning-panel-steps" className="flex flex-col pt-2.5 pb-1">
        {steps.map((step, i) => {
          const last = i === steps.length - 1;
          const active = streaming && last;
          return (
            <li
              key={i}
              data-slot="reasoning-panel-step"
              data-active={active ? '' : undefined}
              className="fade-in slide-in-from-bottom-1 animate-in fill-mode-both relative flex gap-3 pb-3.5 duration-300 last:pb-0">
              {/* The timeline: a dot per step joined by a hairline rail. */}
              {!last && (
                <span
                  aria-hidden
                  className="bg-foreground/10 absolute top-[15px] bottom-0 start-[2px] w-px"
                />
              )}
              <span
                aria-hidden
                className={cn(
                  'relative mt-[7px] size-[5px] shrink-0 rounded-full transition-colors duration-300',
                  active ? 'bg-primary-500 motion-safe:animate-pulse' : 'bg-foreground/25'
                )}
              />
              <div className="flex min-w-0 flex-1 flex-col">
                <p
                  data-slot="reasoning-step-title"
                  className="text-foreground/85 text-[13.5px] leading-snug font-medium">
                  {step.title}
                </p>
                {step.body ? <StepBody body={step.body} /> : null}
              </div>
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

  // A streaming → settled transition collapses the panel (when the reader has
  // not taken over), which is an animation the host may need to scroll-lock.
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
        data-slot="reasoning-root"
        data-variant="static"
        data-streaming={streaming ? '' : undefined}
        data-testid={testId}
        aria-busy={streaming || undefined}
        className={cn('w-full', className)}>
        <div
          data-slot="reasoning-panel-header"
          className="text-foreground/55 flex items-center gap-1.5 py-1 text-[13.5px]">
          {label}
        </div>
        {shown.length > 0 && <StepList steps={shown} streaming={streaming} bounded={streaming} />}
      </div>
    );
  }

  return (
    <Collapsible
      data-slot="reasoning-root"
      data-variant="collapsible"
      data-streaming={streaming ? '' : undefined}
      data-testid={testId}
      open={isOpen}
      onOpenChange={handleOpenChange}
      className={cn('w-full', className)}>
      <CollapsibleTrigger
        data-slot="reasoning-panel-trigger"
        disabled={shown.length === 0}
        className="group/trigger text-foreground/55 hover:text-foreground/90 flex max-w-full items-center gap-1.5 rounded-sm py-1 text-[13.5px] transition-[color,scale] outline-none focus-visible:ring-1 focus-visible:ring-foreground/20 active:scale-[0.98] disabled:pointer-events-none">
        {label}
        {shown.length > 0 && (
          <ChevronDownIcon
            aria-hidden
            className="size-3.5 shrink-0 opacity-60 transition-transform duration-200 ease-[cubic-bezier(0.32,0.72,0,1)] group-data-[state=open]/trigger:rotate-180 motion-reduce:transition-none"
          />
        )}
      </CollapsibleTrigger>
      <CollapsibleContent
        data-slot="reasoning-panel-content"
        aria-busy={streaming || undefined}
        className={cn(collapsePanel, 'outline-none')}>
        <StepList steps={shown} streaming={streaming} bounded />
      </CollapsibleContent>
    </Collapsible>
  );
}
