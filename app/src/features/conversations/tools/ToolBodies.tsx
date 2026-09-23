/**
 * Rich bodies for a tool call's expanded row, one per {@link ToolBodyKind}.
 *
 * Each body renders from data the call actually produced (arguments, result
 * text, the core's structured payload) and returns `null` when that data is
 * not there, so the caller falls back to the generic Input/Output view rather
 * than showing an empty frame.
 */
import { SearchIcon } from 'lucide-react';
import { useState } from 'react';

import { Source } from '../../../components/ai-elements';
import { cn } from '../../../components/assistant-ui/lib/utils';
import { useT } from '../../../lib/i18n/I18nContext';
import { BubbleMarkdown } from '../components/AgentMessageBubble';
import { displayUrl, type ToolArgs } from './toolChips';
import { parseWebSearchResult, type WebSearchHit } from './parseWebSearchResult';
import { fillPlaceholders } from './toolPhrases';

const INITIAL_VISIBLE_RESULTS = 4;

/** Deterministic, theme-safe tint for a domain's letter avatar. */
function avatarHue(domain: string): number {
  let hash = 0;
  for (let i = 0; i < domain.length; i += 1) hash = (hash * 31 + domain.charCodeAt(i)) >>> 0;
  return hash % 360;
}

function DomainAvatar({ domain }: { domain: string }) {
  const hue = avatarHue(domain);
  return (
    <span
      aria-hidden
      className="flex size-5 shrink-0 items-center justify-center rounded-full text-[10px] font-semibold uppercase"
      style={{
        backgroundColor: `hsl(${hue} 70% 50% / 0.15)`,
        color: `hsl(${hue} 60% 45%)`,
      }}>
      {domain.charAt(0)}
    </span>
  );
}

function SearchHitRow({ hit, index }: { hit: WebSearchHit; index: number }) {
  return (
    <li
      className="animate-in fade-in-0 slide-in-from-top-1 fill-mode-both duration-300"
      style={{ animationDelay: `${Math.min(index, 6) * 50}ms` }}>
      <Source
        href={hit.url}
        rel="noreferrer noopener"
        data-testid="web-search-hit"
        className="hover:bg-muted/60 flex items-start gap-2.5 rounded-lg px-2 py-1.5 transition-colors">
        <DomainAvatar domain={hit.domain} />
        <span className="min-w-0 flex-1">
          <span className="text-foreground line-clamp-1 text-[13px] font-medium">{hit.title}</span>
          <span className="text-muted-foreground flex min-w-0 items-center gap-1.5 text-[11px]">
            <span className="truncate font-mono">{hit.domain}</span>
            {hit.published ? <span className="shrink-0">· {hit.published}</span> : null}
          </span>
          {hit.excerpt ? (
            <span className="text-muted-foreground mt-0.5 line-clamp-2 text-xs">{hit.excerpt}</span>
          ) : null}
        </span>
      </Source>
    </li>
  );
}

/**
 * The web-search element: query pill, a status line ("Searching…", "Found 6
 * results via Exa") and the hits, each with a domain-letter avatar. No
 * favicons are fetched, so rendering a result never contacts the result's
 * site.
 */
export function WebSearchResults({
  args,
  result,
  structured,
  searching,
}: {
  args: ToolArgs;
  result: unknown;
  structured?: unknown;
  searching: boolean;
}) {
  const { t } = useT();
  const [expanded, setExpanded] = useState(false);
  const parsed = searching ? undefined : parseWebSearchResult(result, structured);
  if (!searching && !parsed) return null;

  const argQuery = typeof args.query === 'string' ? args.query : undefined;
  const query = parsed?.query ?? argQuery;
  const hits = parsed?.results ?? [];
  const visible = expanded ? hits : hits.slice(0, INITIAL_VISIBLE_RESULTS);
  const hidden = hits.length - visible.length;

  let status: string;
  if (searching) status = t('conversations.tools.search.searching', 'Searching…');
  else if (hits.length === 0) status = t('conversations.tools.search.none', 'No results');
  else
    status = fillPlaceholders(
      hits.length === 1
        ? t('conversations.tools.search.found.one', 'Found {count} result')
        : t('conversations.tools.search.found.other', 'Found {count} results'),
      { count: String(hits.length) }
    );
  const via = parsed?.provider
    ? fillPlaceholders(t('conversations.tools.search.via', 'via {provider}'), {
        provider: parsed.provider,
      })
    : undefined;

  return (
    <div data-testid="web-search-results" className="space-y-2">
      {query ? (
        <div
          data-testid="web-search-query"
          className="bg-muted/60 text-foreground flex min-w-0 items-center gap-2 rounded-full px-3 py-1.5 text-xs">
          <SearchIcon aria-hidden className="text-muted-foreground size-3.5 shrink-0" />
          <span className="truncate">{query}</span>
        </div>
      ) : null}
      <p className="text-muted-foreground px-1 text-[11px]" data-testid="web-search-status">
        <span className={cn(searching && 'tool-shimmer')}>{status}</span>
        {via ? <span> · {via}</span> : null}
      </p>
      {visible.length > 0 ? (
        <ul className="space-y-0.5">
          {visible.map((hit, index) => (
            <SearchHitRow key={hit.url} hit={hit} index={index} />
          ))}
        </ul>
      ) : null}
      {hidden > 0 || expanded ? (
        <button
          type="button"
          onClick={() => setExpanded(value => !value)}
          className="text-muted-foreground hover:text-foreground px-2 text-[11px] font-medium">
          {expanded
            ? t('conversations.tools.search.showLess', 'Show less')
            : fillPlaceholders(t('conversations.tools.search.showMore', 'Show {count} more'), {
                count: String(hidden),
              })}
        </button>
      ) : null}
    </div>
  );
}

function stringOf(value: unknown): string | undefined {
  if (typeof value === 'string') return value;
  if (value === undefined || value === null) return undefined;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

/** Terminal-styled command and its output. */
export function ShellBody({ args, result }: { args: ToolArgs; result: unknown }) {
  const command =
    (typeof args.command === 'string' && args.command) ||
    (typeof args.subcommand === 'string' && `npm ${args.subcommand}`) ||
    (typeof args.script_path === 'string' && args.script_path) ||
    (typeof args.inline_code === 'string' && args.inline_code) ||
    undefined;
  const output = stringOf(result)?.trimEnd();
  if (!command && !output) return null;
  return (
    <div
      data-testid="tool-body-shell"
      className="overflow-hidden rounded-lg bg-zinc-950 font-mono text-[11.5px] leading-relaxed text-zinc-100">
      {command ? (
        <pre className="border-b border-white/10 px-3 py-2 whitespace-pre-wrap break-all">
          <span className="text-emerald-400 select-none">$ </span>
          {command}
        </pre>
      ) : null}
      {output ? (
        <pre className="max-h-64 overflow-auto px-3 py-2 whitespace-pre-wrap break-all text-zinc-300">
          {output}
        </pre>
      ) : null}
    </div>
  );
}

/** `status=200 url=https://… content=markdown` header, then the page. */
function splitFetchOutput(text: string): { status?: string; url?: string; body: string } {
  const newline = text.indexOf('\n');
  const head = newline === -1 ? text : text.slice(0, newline);
  if (!/^status=\d{3}\b/.test(head)) return { body: text };
  return {
    status: head.match(/^status=(\d{3})/)?.[1],
    url: head.match(/\burl=(\S+)/)?.[1],
    body: newline === -1 ? '' : text.slice(newline + 1),
  };
}

/** A fetched page: status, where it came from, and the start of its content. */
export function FetchBody({ args, result }: { args: ToolArgs; result: unknown }) {
  const text = typeof result === 'string' ? result : undefined;
  if (!text) return null;
  const { status, url, body } = splitFetchOutput(text);
  const source = url ?? (typeof args.url === 'string' ? args.url : undefined);
  const ok = status ? Number(status) < 400 : true;
  return (
    <div data-testid="tool-body-fetch" className="space-y-1.5">
      {status || source ? (
        <div className="text-muted-foreground flex min-w-0 items-center gap-2 text-[11px]">
          {status ? (
            <span
              className={cn(
                'rounded px-1.5 py-0.5 font-mono font-medium',
                ok
                  ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                  : 'bg-red-500/10 text-red-600 dark:text-red-400'
              )}>
              {status}
            </span>
          ) : null}
          {source ? <span className="truncate font-mono">{displayUrl(source)}</span> : null}
        </div>
      ) : null}
      {body.trim() ? (
        <div className="bg-muted/40 max-h-56 overflow-auto rounded-md px-2.5 py-2 text-xs">
          <BubbleMarkdown content={body.slice(0, 4000)} />
        </div>
      ) : null}
    </div>
  );
}

/** What changed in a file: removed and added text for an edit, else the content. */
export function FileBody({ args, result }: { args: ToolArgs; result: unknown }) {
  const oldText = typeof args.old_string === 'string' ? args.old_string : undefined;
  const newText = typeof args.new_string === 'string' ? args.new_string : undefined;
  if (oldText !== undefined || newText !== undefined) {
    return (
      <div
        data-testid="tool-body-file-diff"
        className="overflow-hidden rounded-lg border font-mono text-[11.5px] leading-relaxed">
        {oldText ? (
          <pre className="max-h-40 overflow-auto bg-red-500/10 px-3 py-1.5 whitespace-pre-wrap text-red-700 dark:text-red-300">
            {oldText
              .split('\n')
              .map(line => `- ${line}`)
              .join('\n')}
          </pre>
        ) : null}
        {newText ? (
          <pre className="max-h-40 overflow-auto bg-emerald-500/10 px-3 py-1.5 whitespace-pre-wrap text-emerald-700 dark:text-emerald-300">
            {newText
              .split('\n')
              .map(line => `+ ${line}`)
              .join('\n')}
          </pre>
        ) : null}
      </div>
    );
  }
  const content =
    typeof args.content === 'string' ? args.content : typeof result === 'string' ? result : '';
  if (!content.trim()) return null;
  return (
    <pre
      data-testid="tool-body-file"
      className="bg-muted/50 max-h-64 overflow-auto rounded-lg px-3 py-2 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap">
      {content.slice(0, 6000)}
    </pre>
  );
}
