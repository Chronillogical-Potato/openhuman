'use client';

/**
 * Vendored from the assistant-ui `message-timing` registry item
 * (https://r.assistant-ui.com/styles/base-nova/message-timing.json).
 * Changes from upstream:
 * - `cn` import path (`@/components/assistant-ui/lib/utils`).
 */
import { cn } from '@/components/assistant-ui/lib/utils';
import type { ComponentProps } from 'react';

import { mono } from './surfaces';

export interface TimingStat {
  label: string;
  value: string;
}

export function MessageTiming({
  stats,
  streaming,
  className,
  ...props
}: Omit<ComponentProps<'div'>, 'children' | 'stats' | 'streaming'> & {
  stats: readonly TimingStat[];
  streaming?: boolean;
}) {
  return (
    <div
      data-slot="message-timing"
      className={cn(
        'fade-in animate-in flex w-full max-w-sm flex-wrap items-center gap-x-3 gap-y-1 duration-500',
        className
      )}
      {...props}>
      {stats.map(stat => (
        <span key={stat.label} className="flex items-baseline gap-1">
          <span className={cn(mono, 'text-foreground/25')}>{stat.label}</span>
          <span
            className={cn(
              mono,
              'tabular-nums',
              streaming ? 'text-blue-500 dark:text-blue-400' : 'text-foreground/50'
            )}>
            {stat.value}
          </span>
        </span>
      ))}
    </div>
  );
}
