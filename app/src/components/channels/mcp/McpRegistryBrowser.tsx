/**
 * Registry — browse the upstream MCP directories.
 *
 * The third tab of the MCP page. It is a *directory*, not a store: nothing
 * here installs. A row opens the server's own page in the browser — its
 * website, or its source repository — where the install instructions live;
 * the user then declares the server in the **mcp.json** tab. That keeps one
 * install path (the document) instead of a form here that would have to guess
 * at each server's launch command and credentials.
 *
 * Everything here is fenced inside this component's own state. The directories
 * are a network hop away and can be down; that is rendered as a notice inside
 * this panel with a retry, while the user's installed servers in the other
 * tabs go on rendering. A dead directory is not a broken page.
 */
import debug from 'debug';
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { useDebouncedValue } from '../../../hooks/useDebouncedValue';
import { useT } from '../../../lib/i18n/I18nContext';
import { mcpClientsApi } from '../../../services/api/mcpClientsApi';
import { openUrl } from '../../../utils/openUrl';
import Button from '../../ui/Button';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../../ui/Table';
import TextField from '../../ui/TextField';
import { mcpRegistryErrorMessage } from './mcpRegistryErrorMessage';
import { deriveAuthor } from './McpServerCard';
import type { SmitheryServer } from './types';

const log = debug('mcp-clients:registry');
const DEBOUNCE_MS = 300;
const PAGE_SIZE = 30;

/** Transport classification a catalog row can be filtered by. */
export type Transport = 'hosted' | 'stdio';

/**
 * Classify a catalog row by how it runs: `hosted` = reachable over an HTTP
 * endpoint; `stdio` = run on-device as a subprocess. `is_deployed` is set by
 * the registry adapter when the server exposes a remote.
 */
const transportOf = (server: SmitheryServer): Transport =>
  server.is_deployed ? 'hosted' : 'stdio';

/**
 * Derive a browsable source-repository URL from the registry slug. The official
 * registry namespaces community servers as `io.github.<user>/<repo>` (and
 * `io.gitlab.<user>/…`), which maps 1:1 to a repo page. Returns `null` for
 * vendor reverse-DNS slugs that don't encode a code host.
 */
export const deriveRepoUrl = (qualifiedName: string): string | null => {
  const slash = qualifiedName.indexOf('/');
  if (slash < 1) return null;
  const prefix = qualifiedName.slice(0, slash);
  const repo = qualifiedName.slice(slash + 1);
  if (!repo) return null;
  if (prefix.startsWith('io.github.')) {
    return `https://github.com/${prefix.slice('io.github.'.length)}/${repo}`;
  }
  if (prefix.startsWith('io.gitlab.')) {
    return `https://gitlab.com/${prefix.slice('io.gitlab.'.length)}/${repo}`;
  }
  return null;
};

/**
 * The page a row opens: the server's own site when it declares one, else its
 * repository, else its listing on the directory it came from. Every row has
 * *somewhere* to go, because a directory entry the user cannot read is not
 * one they can act on.
 */
export const serverPageUrl = (server: SmitheryServer): string => {
  if (server.website_url) return server.website_url;
  const repo = deriveRepoUrl(server.qualified_name);
  if (repo) return repo;
  if (server.source === 'smithery') {
    return `https://smithery.ai/server/${server.qualified_name}`;
  }
  return `https://registry.modelcontextprotocol.io/?search=${encodeURIComponent(
    server.qualified_name
  )}`;
};

/**
 * Collapse catalog entries to a single row per `qualified_name`. The registry
 * can return the same server within a page or across paginated "load more"
 * fetches; first occurrence wins so the earliest (highest-ranked) result is
 * the one kept.
 */
const dedupeByQualifiedName = (servers: SmitheryServer[]): SmitheryServer[] => {
  const seen = new Set<string>();
  const out: SmitheryServer[] = [];
  for (const server of servers) {
    if (seen.has(server.qualified_name)) continue;
    seen.add(server.qualified_name);
    out.push(server);
  }
  return out;
};

/** Transport pill (Stdio vs Hosted) — the catalog's primary classification. */
const TransportBadge = ({ transport }: { transport: Transport }) => {
  const { t } = useT();
  const hosted = transport === 'hosted';
  return (
    <span
      title={t(hosted ? 'mcp.tab.transport.hostedHint' : 'mcp.tab.transport.localHint')}
      className={`inline-flex items-center rounded-full px-1.5 py-0.5 text-[10px] font-medium ${
        hosted
          ? 'bg-primary-100 text-primary-700 dark:bg-primary-500/15 dark:text-primary-300'
          : 'bg-surface-strong text-content-muted'
      }`}>
      {t(hosted ? 'mcp.tab.transport.hosted' : 'mcp.tab.transport.local')}
    </span>
  );
};

/**
 * External link that opens in the system browser. Stops propagation so clicking
 * a server's website/repo never also triggers the row's own open action.
 */
const ExternalLink = ({ href, label }: { href: string; label: string }) => (
  <Button
    variant="tertiary"
    size="xs"
    onClick={e => {
      e.stopPropagation();
      void openUrl(href).catch(() => {});
    }}
    className="h-auto gap-0.5 p-0 text-[11px] font-normal text-primary-600 hover:underline dark:text-primary-400">
    {label}
    <svg
      className="w-2.5 h-2.5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth={2}
      aria-hidden="true">
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
      />
    </svg>
  </Button>
);

/**
 * One catalog row. Memoized so a parent re-render (the installed list's status
 * poll) doesn't re-render the potentially large catalog: the data and the open
 * handler are stable, so memo skips every row.
 */
const CatalogRow = memo(
  ({ server, onOpen }: { server: SmitheryServer; onOpen: (server: SmitheryServer) => void }) => {
    const { t } = useT();
    const repoUrl = deriveRepoUrl(server.qualified_name);
    const author = deriveAuthor(server.qualified_name);
    return (
      <TableRow
        className="cursor-pointer"
        tabIndex={0}
        role="button"
        data-testid="mcp-registry-row"
        aria-label={t('mcp.tab.aria.openServerPage').replace('{name}', server.display_name)}
        onClick={() => onOpen(server)}
        onKeyDown={e => {
          // Only act on keys aimed at the row itself — Enter/Space bubble up
          // from the nested Website/Repository buttons.
          if (e.target !== e.currentTarget) return;
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onOpen(server);
          }
        }}>
        <TableCell>
          <div className="flex items-center gap-2.5">
            {server.icon_url ? (
              <img
                src={server.icon_url}
                alt=""
                className="w-5 h-5 rounded shrink-0 object-contain"
              />
            ) : (
              <span className="w-5 h-5 rounded shrink-0 bg-primary-100 dark:bg-primary-500/20 flex items-center justify-center text-[10px]">
                🔌
              </span>
            )}
            <div className="min-w-0">
              <span className="flex items-center gap-1.5">
                <span className="font-medium text-content truncate">{server.display_name}</span>
                {server.official && (
                  <span
                    title={t('mcp.tab.officialHint')}
                    className="inline-flex items-center gap-0.5 rounded-full bg-sage-100 px-1.5 py-0.5 text-[10px] font-medium text-sage-700 dark:bg-sage-500/15 dark:text-sage-300">
                    ✓ {t('mcp.tab.officialBadge')}
                  </span>
                )}
              </span>
              {/* The registry is full of look-alike names (a dozen "gmail"
                  servers); the slug is the unique identifier that tells them
                  apart. */}
              <span className="text-[11px] font-mono text-content-faint truncate block">
                {server.qualified_name}
              </span>
              {server.description && (
                <span className="text-xs text-content-faint line-clamp-3 block">
                  {server.description}
                </span>
              )}
              {(server.website_url || repoUrl) && (
                <span className="flex items-center gap-3 mt-1">
                  {server.website_url && (
                    <ExternalLink href={server.website_url} label={t('mcp.tab.link.website')} />
                  )}
                  {repoUrl && <ExternalLink href={repoUrl} label={t('mcp.tab.link.repo')} />}
                </span>
              )}
            </div>
          </div>
        </TableCell>
        <TableCell className="hidden sm:table-cell">
          <TransportBadge transport={transportOf(server)} />
        </TableCell>
        <TableCell className="hidden sm:table-cell">
          <span className="text-xs text-content-muted truncate block">{author ?? '—'}</span>
        </TableCell>
        <TableCell className="text-right">
          <span className="text-xs text-primary-600 dark:text-primary-400 font-medium">
            {t('mcp.tab.action.openPage')} ↗
          </span>
        </TableCell>
      </TableRow>
    );
  }
);
CatalogRow.displayName = 'CatalogRow';

interface McpRegistryBrowserProps {
  /** Names already declared, so the directory does not list what is installed. */
  installedNames: ReadonlySet<string>;
}

const McpRegistryBrowser = ({ installedNames }: McpRegistryBrowserProps) => {
  const { t } = useT();
  const [searchQuery, setSearchQuery] = useState('');
  const [transportFilter, setTransportFilter] = useState<'all' | Transport>('all');
  const filters = useMemo(
    () => ({ query: searchQuery, transport: transportFilter }),
    [searchQuery, transportFilter]
  );
  const debouncedFilters = useDebouncedValue(filters, DEBOUNCE_MS);

  const [catalogServers, setCatalogServers] = useState<SmitheryServer[]>([]);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [catalogPage, setCatalogPage] = useState(1);
  const [catalogTotalPages, setCatalogTotalPages] = useState(1);
  // Set when a fetch fails so the tab shows an error state (with retry)
  // instead of silently falling back to an empty/stale catalog.
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const requestSeqRef = useRef(0);

  const fetchCatalog = useCallback(
    async (query: string, transport: 'all' | Transport, page: number, append: boolean) => {
      const seq = ++requestSeqRef.current;
      setCatalogLoading(true);
      try {
        const result = await mcpClientsApi.registrySearch({
          query: query || undefined,
          transport: transport === 'all' ? undefined : transport,
          page,
          page_size: PAGE_SIZE,
        });
        if (seq !== requestSeqRef.current) return;
        const incoming = result.servers ?? [];
        setCatalogServers(prev =>
          dedupeByQualifiedName(append ? [...prev, ...incoming] : incoming)
        );
        setCatalogPage(result.page);
        setCatalogTotalPages(result.total_pages);
        setCatalogError(null);
      } catch (err) {
        if (seq !== requestSeqRef.current) return;
        log('catalog fetch error: %o', err);
        // A fresh (non-append) fetch that fails leaves no usable rows — surface
        // the error. A failed "load more" keeps the rows already shown.
        if (!append) setCatalogError(mcpRegistryErrorMessage(err, t, 'mcp.catalog.loadFailed'));
      } finally {
        if (seq === requestSeqRef.current) setCatalogLoading(false);
      }
    },
    [t]
  );

  // Fetch page 1 on mount and whenever the query or transport filter changes.
  useEffect(() => {
    void fetchCatalog(debouncedFilters.query, debouncedFilters.transport, 1, false);
  }, [debouncedFilters, fetchCatalog]);

  const handleOpen = useCallback((server: SmitheryServer) => {
    const url = serverPageUrl(server);
    log('opening server page %s', url);
    void openUrl(url).catch(() => {});
  }, []);

  // Catalog rows minus already-declared servers.
  const availableCatalog = useMemo(
    () => catalogServers.filter(s => !installedNames.has(s.qualified_name)),
    [catalogServers, installedNames]
  );

  const catalogRows = useMemo(
    () =>
      availableCatalog.map(server => (
        <CatalogRow key={`catalog-${server.qualified_name}`} server={server} onOpen={handleOpen} />
      )),
    [availableCatalog, handleOpen]
  );

  return (
    <div className="space-y-3" data-testid="mcp-registry-browser">
      <p className="text-xs text-content-muted">{t('mcp.registry.intro')}</p>

      <div className="flex flex-wrap items-center gap-2">
        <TextField
          type="search"
          value={searchQuery}
          onChange={e => setSearchQuery(e.target.value)}
          placeholder={t('mcp.catalog.searchPlaceholder')}
          aria-label={t('mcp.catalog.searchAria')}
          className="flex-1 min-w-48"
        />
        <span className="text-xs font-medium text-content-muted">
          {t('mcp.tab.transportFilter.label')}
        </span>
        <div
          className="flex flex-wrap items-center gap-2"
          role="group"
          aria-label={t('mcp.tab.transportFilter.aria')}>
          {(['stdio', 'hosted'] as const).map(tp => {
            const active = transportFilter === tp;
            return (
              <Button
                key={tp}
                variant={active ? 'primary' : 'secondary'}
                size="xs"
                aria-pressed={active}
                onClick={() => setTransportFilter(prev => (prev === tp ? 'all' : tp))}
                className={`rounded-full font-medium ${active ? 'bg-content text-surface' : ''}`}>
                {t(tp === 'stdio' ? 'mcp.tab.transport.local' : 'mcp.tab.transport.hosted')}
              </Button>
            );
          })}
        </div>
      </div>

      <Table className="min-w-[640px] rounded-lg border border-line">
        <TableHeader>
          <TableRow className="bg-surface-muted">
            <TableHead>{t('mcp.tab.column.name')}</TableHead>
            <TableHead className="hidden w-28 sm:table-cell">{t('mcp.tab.column.type')}</TableHead>
            <TableHead className="hidden w-36 sm:table-cell">
              {t('mcp.tab.column.author')}
            </TableHead>
            <TableHead className="w-28 text-right">{t('mcp.tab.column.action')}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>{catalogRows}</TableBody>
      </Table>

      <div className="rounded-b-lg border border-t-0 border-line">
        {catalogError && !catalogLoading && (
          <div
            data-testid="mcp-catalog-error"
            className="py-8 text-center text-sm text-coral-700 dark:text-coral-300 space-y-2">
            <p>{catalogError}</p>
            <Button
              variant="tertiary"
              size="xs"
              onClick={() =>
                void fetchCatalog(debouncedFilters.query, debouncedFilters.transport, 1, false)
              }
              className="text-primary-600 dark:text-primary-400 hover:underline">
              {t('common.retry')}
            </Button>
          </div>
        )}

        {availableCatalog.length === 0 && !catalogLoading && !catalogError && (
          <div
            data-testid="mcp-catalog-empty"
            className="py-8 text-center text-sm text-content-faint">
            {searchQuery
              ? t('mcp.catalog.noResultsFor').replace('{query}', searchQuery)
              : t('mcp.catalog.noResults')}
          </div>
        )}

        {catalogLoading && (
          <div className="py-4 text-center text-xs text-content-faint">{t('common.loading')}</div>
        )}
        {!catalogLoading && catalogPage < catalogTotalPages && (
          <div className="py-3 text-center border-t border-line-subtle">
            <Button
              variant="tertiary"
              size="xs"
              onClick={() =>
                void fetchCatalog(
                  debouncedFilters.query,
                  debouncedFilters.transport,
                  catalogPage + 1,
                  true
                )
              }
              className="text-primary-600 dark:text-primary-400 hover:underline">
              {t('mcp.catalog.loadMore')}
            </Button>
          </div>
        )}
      </div>
    </div>
  );
};

export default McpRegistryBrowser;
