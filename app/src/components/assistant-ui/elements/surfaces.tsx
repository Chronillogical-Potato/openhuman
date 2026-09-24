'use client';

/**
 * Shared surface primitives for the assistant-ui elements, vendored from the
 * `elements-surfaces` registry item. Only the pieces the reasoning panel uses
 * are kept. `collapsePanel` is adapted from Base UI's height variable to the
 * Radix `Collapsible` this app ships (`tw-animate-css` provides the
 * `collapsible-down/up` keyframes over `--radix-collapsible-content-height`).
 */
import { cn } from '@/components/assistant-ui/lib/utils';
import { type ComponentProps, type ReactNode, useLayoutEffect, useRef, useState } from 'react';

export const labelSwap =
  'col-start-1 row-start-1 flex w-max items-center gap-1.5 leading-none transition-[opacity,filter] duration-300 ease-[cubic-bezier(0.23,1,0.32,1)] motion-reduce:transition-none';

export const labelSwapIn = 'opacity-100 blur-none';

export const labelSwapOut = 'pointer-events-none select-none opacity-0 blur-[2px]';

export const collapsePanel =
  'overflow-hidden ease-[cubic-bezier(0.32,0.72,0,1)] data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down motion-reduce:animate-none';

export const mono = 'font-mono text-[11px] tracking-tight';

export function ShimmerLabel({
  active = true,
  className,
  ...props
}: ComponentProps<'span'> & { active?: boolean }) {
  return (
    <span
      data-shimmer={active ? '' : undefined}
      className={cn(active && 'shimmer motion-reduce:animate-none', className)}
      {...props}
    />
  );
}

/**
 * Two stacked labels that cross-fade, with the wrapper's width animating to
 * whichever layer is active, so "Thinking… 4s" settling into "Thought for 12s"
 * slides rather than jumps.
 */
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
  const [width, setWidth] = useState<number | null>(null);

  useLayoutEffect(() => {
    const target = (active === 0 ? first : second).current;
    if (!target) return undefined;
    const measure = () => setWidth(Math.ceil(target.getBoundingClientRect().width));
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(target);
    return () => observer.disconnect();
  }, [active]);

  return (
    <span
      style={width ? { width } : undefined}
      className={cn(
        'grid overflow-x-clip transition-[width] duration-300 ease-[cubic-bezier(0.23,1,0.32,1)] motion-reduce:transition-none',
        className
      )}>
      {children.map((layer, index) => (
        <span
          key={index}
          ref={index === 0 ? first : second}
          aria-hidden={active !== index}
          data-swap-layer={index === 0 ? 'live' : 'resting'}
          data-active={active === index ? '' : undefined}
          className={cn(labelSwap, active === index ? labelSwapIn : labelSwapOut)}>
          {layer}
        </span>
      ))}
    </span>
  );
}
