'use client';

/**
 * Shared design tokens for the assistant-ui elements in this folder.
 *
 * Vendored from assistant-ui `packages/ui/src/components/react/assistant-ui/elements/surfaces.tsx`
 * (commit 1abca347). Changes from upstream, kept minimal so a re-sync stays a
 * small diff:
 * - `cn` import path.
 * - `collapsePanel` drives the Radix collapsible this app uses (upstream
 *   targets Base UI's `--collapsible-panel-height`).
 * - `openRotate` added: the Radix trigger reports `data-state=open`, not Base
 *   UI's `data-open` / `data-panel-open`.
 */
import type { ComponentProps, ReactNode } from 'react';
import { useLayoutEffect, useRef, useState } from 'react';

import { cn } from '@/components/assistant-ui/lib/utils';

export const paper = 'bg-background border border-border/60 dark:bg-popover';

export const floating = 'bg-background border border-border/60 dark:bg-popover';

export const field = 'bg-foreground/[0.04] dark:bg-foreground/[0.06]';

export const fieldInteractive =
  'bg-foreground/[0.04] transition-colors hover:bg-foreground/[0.07] dark:bg-foreground/[0.06] dark:hover:bg-foreground/[0.09]';

export const pressable =
  'transition-transform duration-150 ease-[cubic-bezier(0.23,1,0.32,1)] active:scale-[0.96] motion-reduce:transition-none';

export const ghostButton =
  'flex items-center justify-center rounded-full text-foreground/45 outline-none transition-[background-color,color,scale] duration-150 hover:bg-foreground/[0.06] hover:text-foreground/90 active:scale-[0.96] focus-visible:ring-1 focus-visible:ring-foreground/20 motion-reduce:transition-none dark:hover:bg-foreground/[0.09]';

export const labelSwap =
  'col-start-1 row-start-1 flex w-max items-center gap-1.5 leading-none transition-[opacity,filter] duration-300 ease-[cubic-bezier(0.23,1,0.32,1)] motion-reduce:transition-none';

export const labelSwapIn = 'opacity-100 blur-none';

export const labelSwapOut = 'pointer-events-none select-none opacity-0 blur-[2px]';

export const collapsePanel =
  'overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down motion-reduce:animate-none';

export const openRotate =
  'transition-transform duration-200 ease-[cubic-bezier(0.32,0.72,0,1)] group-data-[state=open]/trigger:rotate-90 motion-reduce:transition-none';

export const live = 'text-blue-500 dark:text-blue-400';

export const mono = 'font-mono text-[11px] tracking-tight';

export function ShimmerLabel({
  active = true,
  className,
  ...props
}: ComponentProps<'span'> & { active?: boolean }) {
  return (
    <span className={cn(active && 'shimmer motion-reduce:animate-none', className)} {...props} />
  );
}

/**
 * Scroll region for content that keeps its own whitespace. `whitespace-pre` in
 * a bounded box clips a long line with no way to reach it, so the rows scroll
 * instead.
 *
 * `codeSurface` wraps all the rows as one block, and the rows are its children.
 * It cannot go on each row: `min-width: 100%` resolves against the scroll
 * container's visible width rather than its scroll width, so a per-row width
 * leaves every row except the longest ending its background at the fold.
 */
export const codeScroll = 'overflow-x-auto';

export const codeSurface = 'w-max min-w-full';

export function SwapLabel({
  active,
  children,
  className,
}: {
  active: 0 | 1;
  children: [ReactNode, ReactNode];
  className?: string;
}) {
  const first = useRef<HTMLSpanElement>(null);
  const second = useRef<HTMLSpanElement>(null);
  const layers = [first, second];
  const [width, setWidth] = useState<number | null>(null);

  useLayoutEffect(() => {
    const target = (active === 0 ? first : second).current;
    if (!target) return undefined;
    const measure = () => setWidth(Math.ceil(target.getBoundingClientRect().width));
    measure();
    if (typeof ResizeObserver === 'undefined') return undefined;
    const observer = new ResizeObserver(measure);
    observer.observe(target);
    return () => observer.disconnect();
  }, [active]);

  return (
    <span
      style={width === null ? undefined : { width }}
      className={cn(
        'grid overflow-x-clip transition-[width] duration-300 ease-[cubic-bezier(0.23,1,0.32,1)] motion-reduce:transition-none',
        className
      )}>
      {children.map((layer, index) => (
        <span
          key={index}
          ref={layers[index]}
          aria-hidden={active !== index}
          className={cn(labelSwap, active === index ? labelSwapIn : labelSwapOut)}>
          {layer}
        </span>
      ))}
    </span>
  );
}
