'use client';

/**
 * Vendored from the assistant-ui `elements-message-timing` registry item
 * (https://r.assistant-ui.com/styles/base-nova/elements-message-timing.json).
 * Changes from upstream:
 * - `cn` import path (`@/components/assistant-ui/lib/utils`).
 * - `Tooltip`/`TooltipContent`/`TooltipProvider`/`TooltipTrigger` import path
 *   (`@/components/assistant-ui/ui/tooltip`).
 * - "First token"/"Total"/"Speed"/"Chunks" and the "tok/s" suffix are now
 *   `firstTokenLabel`/`totalLabel`/`speedLabel`/`chunksLabel`/
 *   `tokensPerSecondSuffix` props; the "Message timing" aria-label is
 *   `ariaLabel`; all with English defaults, for `useT()`.
 */
import { cn } from '@/components/assistant-ui/lib/utils';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/assistant-ui/ui/tooltip';
import { useMessageTiming } from '@assistant-ui/react';
import type { FC } from 'react';

const formatTimingMs = (ms: number | undefined): string => {
  if (ms === undefined) return '—';
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
};

/**
 * Shows streaming stats (TTFT, total time, tok/s, chunks) as a badge with a
 * hover/focus tooltip. Renders nothing until the stream completes.
 *
 * Place it inside `ActionBarPrimitive.Root` in your `thread.tsx` so it
 * inherits the action bar's autohide behaviour:
 *
 * ```tsx
 * import { MessageTiming } from "@/components/assistant-ui/elements/message-timing.aui";
 *
 * <ActionBarPrimitive.Root >
 *   <ActionBarPrimitive.Copy />
 *   <ActionBarPrimitive.Reload />
 *   <MessageTiming />  // <-- add this
 * </ActionBarPrimitive.Root>
 * ```
 *
 * @param side - Side of the tooltip relative to the badge trigger.
 * @default "right"
 */
export const MessageTiming: FC<{
  className?: string;
  side?: 'top' | 'right' | 'bottom' | 'left';
  firstTokenLabel?: string;
  totalLabel?: string;
  speedLabel?: string;
  chunksLabel?: string;
  ariaLabel?: string;
  tokensPerSecondSuffix?: string;
  formatTiming?: (ms: number | undefined) => string;
}> = ({
  className,
  side = 'right',
  firstTokenLabel = 'First token',
  totalLabel = 'Total',
  speedLabel = 'Speed',
  chunksLabel = 'Chunks',
  ariaLabel = 'Message timing',
  tokensPerSecondSuffix = 'tok/s',
  formatTiming = formatTimingMs,
}) => {
  const timing = useMessageTiming();
  if (timing?.totalStreamTime === undefined) return null;

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger
          render={
            <button
              type="button"
              data-slot="message-timing-trigger"
              aria-label={ariaLabel}
              className={cn(
                'text-muted-foreground hover:bg-accent hover:text-accent-foreground flex items-center rounded-md p-1 font-mono text-xs tabular-nums transition-colors',
                className
              )}
            />
          }>
          {formatTiming(timing.totalStreamTime)}
        </TooltipTrigger>
        <TooltipContent
          side={side}
          sideOffset={8}
          data-slot="message-timing-popover"
          className="bg-popover text-popover-foreground border px-3 py-2 [&_[data-slot=tooltip-arrow]]:hidden">
          <div className="grid min-w-35 gap-1.5 text-xs">
            {timing.firstTokenTime !== undefined && (
              <div className="flex items-center justify-between gap-4">
                <span className="text-muted-foreground">{firstTokenLabel}</span>
                <span className="font-mono tabular-nums">
                  {formatTiming(timing.firstTokenTime)}
                </span>
              </div>
            )}
            <div className="flex items-center justify-between gap-4">
              <span className="text-muted-foreground">{totalLabel}</span>
              <span className="font-mono tabular-nums">{formatTiming(timing.totalStreamTime)}</span>
            </div>
            {timing.tokensPerSecond !== undefined && (
              <div className="flex items-center justify-between gap-4">
                <span className="text-muted-foreground">{speedLabel}</span>
                <span className="font-mono tabular-nums">
                  {timing.tokensPerSecond.toFixed(1)} {tokensPerSecondSuffix}
                </span>
              </div>
            )}
            <div className="flex items-center justify-between gap-4">
              <span className="text-muted-foreground">{chunksLabel}</span>
              <span className="font-mono tabular-nums">{timing.totalChunks}</span>
            </div>
          </div>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
};
