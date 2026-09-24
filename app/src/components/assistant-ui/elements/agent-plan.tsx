'use client';

/**
 * The agent's step-by-step plan for the current thread, with a progress bar
 * and a checkmark/spinner per step.
 *
 * Vendored from the assistant-ui `elements-agent-plan` registry item
 * (https://r.assistant-ui.com/styles/base-nova/elements-agent-plan.json).
 * Changes from upstream:
 * - `cn` import path (`@/components/assistant-ui/lib/utils`).
 * - The header text `"Plan"` is a prop (`title`) with that English default,
 *   so the caller (`PlanReviewPart` in
 *   `features/conversations/aui/PlanReviewPart.tsx`) supplies the translated
 *   string via `useT()`.
 */
import { cn } from '@/components/assistant-ui/lib/utils';
import { CheckIcon, Loader2Icon } from 'lucide-react';
import type { ComponentProps } from 'react';

import { pct, progressOf } from '../utils/range';
import { mono } from './surfaces';

export function AgentPlan({
  steps,
  activeIndex,
  title = 'Plan',
  className,
  ...props
}: Omit<ComponentProps<'div'>, 'children' | 'steps' | 'activeIndex' | 'title'> & {
  steps: readonly string[];
  activeIndex: number;
  title?: string;
}) {
  const total = steps.length;
  const completed = progressOf(activeIndex, total);
  const allDone = completed >= total;
  const progress = pct(completed, total);

  return (
    <div
      data-slot="agent-plan"
      className={cn('flex w-full max-w-sm flex-col gap-3', className)}
      {...props}>
      <div className="flex items-center justify-between">
        <span className="text-[13.5px] font-medium">{title}</span>
        <span className={cn(mono, 'text-foreground/35 tabular-nums')}>
          {completed} of {total}
        </span>
      </div>
      <div className="bg-foreground/[0.06] h-[3px] w-full overflow-hidden rounded-full">
        <span
          className="bg-foreground/80 block h-full rounded-full transition-[width] duration-500"
          style={{ width: `${progress}%` }}
        />
      </div>
      <ul className="flex flex-col gap-2.5">
        {steps.map((step, i) => {
          const done = allDone || i < completed;
          const active = !allDone && i === completed;
          return (
            <li key={step} className="flex items-center gap-2.5 text-[13.5px]">
              <span className="flex size-4 shrink-0 items-center justify-center">
                {done ? (
                  <CheckIcon className="text-foreground/35 size-3.5" />
                ) : active ? (
                  <Loader2Icon className="text-foreground/90 size-3.5 animate-spin motion-reduce:animate-none" />
                ) : (
                  <span aria-hidden className="bg-foreground/15 size-1.5 rounded-full" />
                )}
              </span>
              <span
                className={cn(
                  done && 'text-foreground/40',
                  active && 'text-foreground/90',
                  !done && !active && 'text-foreground/35'
                )}>
                {step}
              </span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
