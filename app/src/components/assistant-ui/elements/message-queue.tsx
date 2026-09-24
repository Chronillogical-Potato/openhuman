'use client';

/**
 * The running message and the messages queued behind it, each removable.
 *
 * Vendored from the assistant-ui `elements-message-queue` registry item
 * (https://r.assistant-ui.com/styles/base-nova/elements-message-queue.json).
 * Changes from upstream:
 * - `cn` import path (`@/components/assistant-ui/lib/utils`).
 * - The "running", "N queued" and "sends when this finishes" captions and the
 *   remove button's accessible name are props with English defaults, for
 *   `useT()` — see `ComposerMessageQueue` in
 *   `features/conversations/aui/ComposerMessageQueue.tsx`, the only caller.
 */
import { cn } from '@/components/assistant-ui/lib/utils';
import { ArrowUpIcon, XIcon } from 'lucide-react';
import type { ComponentProps } from 'react';

import { field, ghostButton, mono, paper } from './surfaces';

export interface QueuedMessage {
  id: string;
  text: string;
}

export function MessageQueue({
  running,
  queued,
  onCancel,
  runningLabel = 'running',
  queuedLabel = (count: number) => `${count} queued`,
  pendingHint = 'sends when this finishes',
  removeLabel = (text: string) => `Remove "${text}" from the queue`,
  className,
  ...props
}: Omit<ComponentProps<'div'>, 'children' | 'running' | 'queued' | 'onCancel'> & {
  running: string;
  queued: readonly QueuedMessage[];
  onCancel?: (id: string) => void;
  runningLabel?: string;
  queuedLabel?: (count: number) => string;
  pendingHint?: string;
  removeLabel?: (text: string) => string;
}) {
  return (
    <div
      data-slot="message-queue"
      className={cn('flex w-full max-w-sm flex-col gap-2', className)}
      {...props}>
      <div className={cn(paper, 'flex items-center gap-2.5 rounded-2xl p-3')}>
        <span className="relative flex size-2 shrink-0">
          <span className="absolute inline-flex size-full animate-ping rounded-full bg-blue-500/60 motion-reduce:hidden" />
          <span className="relative inline-flex size-2 rounded-full bg-blue-500 dark:bg-blue-400" />
        </span>
        <span className="text-foreground/90 min-w-0 flex-1 truncate text-[13.5px]">{running}</span>
        <span className={cn(mono, 'text-foreground/35 shrink-0')}>{runningLabel}</span>
      </div>

      {queued.length > 0 && (
        <div className="flex items-baseline justify-between px-1">
          <span className={cn(mono, 'text-foreground/35')}>{queuedLabel(queued.length)}</span>
          <span className={cn(mono, 'text-foreground/35')}>{pendingHint}</span>
        </div>
      )}

      <ul className="flex flex-col gap-1.5">
        {queued.map((message, index) => (
          <li
            key={message.id}
            className={cn(
              field,
              'fade-in slide-in-from-bottom-1 animate-in fill-mode-both flex items-center gap-2.5 rounded-2xl py-2 pr-2 pl-3 duration-300'
            )}>
            <span className={cn(mono, 'text-foreground/30 w-3 shrink-0 tabular-nums')}>
              {index + 1}
            </span>
            <span className="text-foreground/60 min-w-0 flex-1 truncate text-[13.5px]">
              {message.text}
            </span>
            <ArrowUpIcon className="text-foreground/25 size-3 shrink-0" />
            {onCancel && (
              <button
                type="button"
                aria-label={removeLabel(message.text)}
                onClick={() => onCancel(message.id)}
                className={cn(ghostButton, 'size-6 shrink-0')}>
                <XIcon className="size-3.5" />
              </button>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
