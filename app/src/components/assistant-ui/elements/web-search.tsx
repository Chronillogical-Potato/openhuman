'use client';

/**
 * assistant-ui's web-search element: the query as a pill, a status line that
 * shimmers while searching, and the hits with a domain-initial avatar.
 *
 * Vendored from assistant-ui `packages/ui/src/components/react/assistant-ui/elements/web-search.tsx`
 * (commit 1abca347). Changes from upstream:
 * - The status line is props (`searchingLabel`, `statusLabel`); upstream
 *   hardcodes "Searching" / "Read 3 sources".
 * - A result may carry a `url`; the row then renders through `renderLink`
 *   so the host decides how an external link opens. A result's `url` is
 *   provider-supplied, so the host must only pass vetted http(s) URLs.
 * - Result keys include the URL: two hits often share a domain.
 * - The empty-results floor (`min-h`) applies only while hits are expected.
 */
import { SearchIcon } from 'lucide-react';
import type { ComponentProps, ReactNode } from 'react';

import { cn } from '@/components/assistant-ui/lib/utils';

import { take } from '../utils/range';
import { field, mono, ShimmerLabel } from './surfaces';

export interface WebSearchResult {
  title: string;
  domain: string;
  url?: string;
}

const rowClass =
  'fade-in slide-in-from-bottom-1 animate-in fill-mode-both hover:bg-foreground/[0.03] -mx-2.5 flex items-center gap-2.5 rounded-xl px-2.5 py-1.5 transition-colors duration-300';

export function WebSearch({
  query,
  results,
  visibleResults,
  searching,
  cycle,
  searchingLabel = 'Searching',
  statusLabel,
  renderLink,
  className,
  ...props
}: Omit<
  ComponentProps<'div'>,
  'children' | 'query' | 'results' | 'visibleResults' | 'searching' | 'cycle'
> & {
  query: string;
  results: readonly WebSearchResult[];
  visibleResults: number;
  searching: boolean;
  cycle: number;
  searchingLabel?: string;
  statusLabel: string;
  renderLink?: (props: { href: string; className: string; children: ReactNode }) => ReactNode;
}) {
  return (
    <div
      data-slot="web-search"
      className={cn('flex w-full max-w-sm flex-col gap-2.5', className)}
      {...props}>
      <span
        data-slot="web-search-query"
        className={cn(
          field,
          'text-foreground/70 inline-flex w-fit max-w-full items-center gap-1.5 rounded-full px-3.5 py-2 text-xs'
        )}>
        <SearchIcon className="text-foreground/40 size-3 shrink-0" />
        <span className="truncate">{query}</span>
      </span>
      <div data-slot="web-search-status" className="text-foreground/45 text-xs">
        {searching ? (
          <ShimmerLabel className="relative inline-block leading-none">
            {searchingLabel}
          </ShimmerLabel>
        ) : (
          <span className="fade-in animate-in duration-300">{statusLabel}</span>
        )}
      </div>
      <div className={cn('flex flex-col', searching && 'min-h-[5.75rem]')}>
        {take(results, visibleResults).map(result => {
          const content = (
            <>
              <span className="bg-foreground/[0.06] text-foreground/45 flex size-4 shrink-0 items-center justify-center rounded text-[9px] font-medium">
                {result.domain.charAt(0).toUpperCase()}
              </span>
              <span className="text-foreground/90 min-w-0 flex-1 truncate text-[13.5px]">
                {result.title}
              </span>
              <span className={cn(mono, 'text-foreground/35 shrink-0')}>{result.domain}</span>
            </>
          );
          const key = `${cycle}-${result.url ?? result.domain}-${result.title}`;
          return result.url && renderLink ? (
            <span key={key} data-slot="web-search-result" className="contents">
              {renderLink({ href: result.url, className: rowClass, children: content })}
            </span>
          ) : (
            <div key={key} data-slot="web-search-result" className={rowClass}>
              {content}
            </div>
          );
        })}
      </div>
    </div>
  );
}
