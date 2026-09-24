'use client';

/**
 * Vendored from the assistant-ui `elements-inline-citation` registry item
 * (https://r.assistant-ui.com/styles/base-nova/elements-inline-citation.json).
 *
 * Upstream ships this as a specimen: one hard-coded paragraph with two
 * citation markers stitched into fixed sentence positions, purely to show the
 * hover-card interaction. That shape does not fit a real caller, which needs
 * to drop a `[n]` marker into arbitrary markdown-rendered text at the offset
 * where the model wrote it. Refactor from the specimen (minimal, allowed
 * change per the vendoring rules):
 * - The fixed `<p>` + two inline `<Citation>` calls is replaced by
 *   `CitationMarker`, a single exported marker component keyed by `index`
 *   into a `sources` array. A caller renders one per `[n]`/`[^n]` token it
 *   finds in the message text (see `markdown-text.tsx`).
 * - `open`/`onOpenChange` become optional; omitted, the marker manages its
 *   own hover-card state (`useState`) so callers with many independent
 *   markers scattered through prose don't have to lift index-keyed state.
 * - `cn` import path (`@/components/assistant-ui/lib/utils`); `./surfaces` ->
 *   sibling `surfaces.tsx` (unchanged import shape).
 * - The upstream `InlineCitationProps`/`InlineCitation` demo wrapper is
 *   dropped; `Source` (renamed `CitationSource` to avoid a name clash with
 *   `sources.aui.tsx`'s `Source`) and `CitationMarker` are the public API.
 */
import { cn } from '@/components/assistant-ui/lib/utils';
import { PreviewCard } from '@base-ui/react/preview-card';
import type { ComponentProps } from 'react';

import { floating, mono } from './surfaces';

export interface CitationSource {
  domain: string;
  title: string;
  snippet: string;
}

export interface CitationMarkerProps extends Omit<
  ComponentProps<'button'>,
  'children' | 'onOpenChange'
> {
  /** 0-based position, rendered as `index + 1`. */
  index: number;
  source: CitationSource;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

/** One `[n]` marker with a hover card showing the cited source. */
export function CitationMarker({
  index,
  source,
  open,
  onOpenChange,
  className,
  ...props
}: CitationMarkerProps) {
  return (
    <PreviewCard.Root open={open} onOpenChange={onOpenChange}>
      <PreviewCard.Trigger
        delay={0}
        render={<button type="button" {...props} />}
        className={cn(
          'mx-0.5 inline-flex h-4 min-w-4 translate-y-[-2px] cursor-default items-center justify-center rounded-[5px] px-1 align-middle font-mono text-[10px] font-medium tabular-nums transition-colors',
          open
            ? 'bg-foreground text-background'
            : 'bg-foreground/[0.06] text-foreground/45 hover:text-foreground/90',
          className
        )}>
        {index + 1}
      </PreviewCard.Trigger>
      <PreviewCard.Portal>
        <PreviewCard.Positioner side="top" sideOffset={8}>
          <PreviewCard.Popup
            className={cn(
              floating,
              'z-50 w-64 origin-(--transform-origin) rounded-2xl p-3.5 outline-none',
              'transition-[opacity,scale] duration-200 ease-[cubic-bezier(0.23,1,0.32,1)] motion-reduce:transition-none',
              'data-[starting-style]:scale-[0.97] data-[starting-style]:opacity-0',
              'data-[ending-style]:scale-[0.97] data-[ending-style]:opacity-0'
            )}>
            <div className="flex items-center gap-1.5">
              <span className="bg-foreground/[0.06] text-foreground/45 flex size-4 items-center justify-center rounded text-[9px] font-medium">
                {source.domain[0]?.toUpperCase()}
              </span>
              <span className={cn(mono, 'text-foreground/40')}>{source.domain}</span>
            </div>
            <p className="mt-2 text-[13px] leading-snug font-medium">{source.title}</p>
            <p className="text-foreground/50 mt-1 text-[13px] leading-relaxed">{source.snippet}</p>
          </PreviewCard.Popup>
        </PreviewCard.Positioner>
      </PreviewCard.Portal>
    </PreviewCard.Root>
  );
}
