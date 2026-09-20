/**
 * Top-level MCP Servers tab: the user's tool servers, said three ways.
 *
 * **Servers** is the list — a row per declared server with its status, and a
 * detail view with the credential form (sign-in or token) and the tool list.
 * **mcp.json** is the same configuration as one document, in the shape a user
 * already has in a desktop config: paste a block of servers, or read the whole
 * set at once instead of expanding rows. **Registry** is the upstream
 * directories, browse-only: a row opens the server's own page, where its
 * install instructions live, and the user declares it in mcp.json.
 *
 * The first two are tabs and not two pages because they are not two things.
 * Both go through the same core RPCs into the same store, so an edit made in
 * one is visible in the other on its next read, and neither is an import
 * format that can drift from "what is actually configured".
 */
import debug from 'debug';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { useT } from '../../../lib/i18n/I18nContext';
import { mcpClientsApi } from '../../../services/api/mcpClientsApi';
import ChipTabs from '../../layout/ChipTabs';
import Button from '../../ui/Button';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../../ui/Table';
import TextField from '../../ui/TextField';
import InstalledServerDetail from './InstalledServerDetail';
import McpConnectionHealthToolbar from './McpConnectionHealthToolbar';
import McpJsonEditor from './McpJsonEditor';
import McpRegistryBrowser from './McpRegistryBrowser';
import { deriveAuthor } from './McpServerCard';
import type { ConnStatus, InstalledServer, ServerStatus } from './types';

const log = debug('mcp-clients:tab');
const POLL_INTERVAL_MS = 5_000;

/** The three notations. `id` doubles as the chip's stable identifier. */
export type McpPageTab = 'servers' | 'json' | 'registry';

type View = { mode: 'home' } | { mode: 'detail'; serverId: string };

/**
 * Collapse installed servers to one row per `qualified_name`. Declaring is
 * idempotent in the core now, but pre-existing double-installs can linger on
 * disk; the first occurrence (earliest install) is kept.
 */
const dedupeInstalledByQualifiedName = (servers: InstalledServer[]): InstalledServer[] => {
  const seen = new Set<string>();
  const out: InstalledServer[] = [];
  for (const server of servers) {
    if (seen.has(server.qualified_name)) continue;
    seen.add(server.qualified_name);
    out.push(server);
  }
  return out;
};

const STATUS_DOT: Record<ServerStatus, string> = {
  connected: 'bg-sage-500',
  connecting: 'bg-amber-400',
  disconnected: 'bg-surface-strong',
  unauthorized: 'bg-amber-500',
  error: 'bg-coral-500',
  disabled: 'bg-surface-strong',
};

/** The dial column: the hosted endpoint's host, or the local command. */
const dialOf = (server: InstalledServer): string => {
  if (server.transport?.kind === 'http_remote') {
    try {
      return new URL(server.transport.url).host;
    } catch {
      return server.transport.url;
    }
  }
  return [server.command, ...server.args].filter(Boolean).join(' ');
};

interface McpServersTabProps {
  /** The tab to open on. Defaults to the server rows. */
  initialTab?: McpPageTab;
}

const McpServersTab = ({ initialTab = 'servers' }: McpServersTabProps) => {
  const { t } = useT();
  const [tab, setTab] = useState<McpPageTab>(initialTab);
  const [servers, setServers] = useState<InstalledServer[]>([]);
  const [statuses, setStatuses] = useState<ConnStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [view, setView] = useState<View>({ mode: 'home' });
  const [searchQuery, setSearchQuery] = useState('');
  const pollTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const loadInstalled = useCallback(async () => {
    log('loading installed servers');
    try {
      const installed = await mcpClientsApi.installedList();
      setServers(Array.isArray(installed) ? installed : []);
      setLoadError(null);
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to load installed servers';
      setLoadError(msg);
    }
  }, []);

  const fetchStatuses = useCallback(async () => {
    try {
      const sv = await mcpClientsApi.status();
      setStatuses(Array.isArray(sv) ? sv : []);
    } catch (err) {
      log('status poll error: %o', err);
    }
  }, []);

  useEffect(() => {
    Promise.all([loadInstalled(), fetchStatuses()]).finally(() => setLoading(false));
  }, [loadInstalled, fetchStatuses]);

  // Poll status while anything is in a non-terminal state — not just
  // `connected`. An `unauthorized`/`error`/`connecting` server can transition
  // (the background reconnect supervisor, a completed OAuth sign-in, an
  // expiring token) and the UI must reflect that without a manual refresh.
  useEffect(() => {
    const hasActive = statuses.some(
      s =>
        s.status === 'connected' ||
        s.status === 'connecting' ||
        s.status === 'unauthorized' ||
        s.status === 'error'
    );
    if (!hasActive) {
      if (pollTimerRef.current) {
        clearTimeout(pollTimerRef.current);
        pollTimerRef.current = null;
      }
      return;
    }
    const schedule = () => {
      pollTimerRef.current = setTimeout(async () => {
        await fetchStatuses();
        schedule();
      }, POLL_INTERVAL_MS);
    };
    schedule();
    return () => {
      if (pollTimerRef.current) {
        clearTimeout(pollTimerRef.current);
        pollTimerRef.current = null;
      }
    };
  }, [statuses, fetchStatuses]);

  const handleSelectServer = useCallback((serverId: string) => {
    setView({ mode: 'detail', serverId });
  }, []);

  const handleUninstalled = useCallback(
    async (_serverId: string) => {
      await loadInstalled();
      await fetchStatuses();
      setView({ mode: 'home' });
    },
    [loadInstalled, fetchStatuses]
  );

  const handleEnabledChange = useCallback(
    async (_serverId: string, _enabled: boolean) => {
      await loadInstalled();
      await fetchStatuses();
    },
    [loadInstalled, fetchStatuses]
  );

  // A save in the document tab adds, rewrites or removes rows; re-read them so
  // the Servers tab describes the configuration as it is after the write. The
  // core connects new servers in the background, so the status poll picks the
  // rest up.
  const handleDocumentSaved = useCallback(async () => {
    await loadInstalled();
    await fetchStatuses();
  }, [loadInstalled, fetchStatuses]);

  // Bulk lifecycle actions for the health toolbar. One failure doesn't abort the
  // batch (allSettled), and we always refresh status so the dots reflect reality
  // — but if any call rejected we then throw so the toolbar can surface the
  // failure (otherwise a partial/total failure would look like success).
  const handleReconnectAll = useCallback(
    async (serverIds: string[]) => {
      log('reconnect all: %o', serverIds);
      const results = await Promise.allSettled(serverIds.map(id => mcpClientsApi.connect(id)));
      await fetchStatuses();
      if (results.some(r => r.status === 'rejected')) {
        throw new Error(t('mcp.health.opErrorGeneric'));
      }
    },
    [fetchStatuses, t]
  );

  const handleDisconnectAll = useCallback(
    async (serverIds: string[]) => {
      log('disconnect all: %o', serverIds);
      const results = await Promise.allSettled(serverIds.map(id => mcpClientsApi.disconnect(id)));
      await fetchStatuses();
      if (results.some(r => r.status === 'rejected')) {
        throw new Error(t('mcp.health.opErrorGeneric'));
      }
    },
    [fetchStatuses, t]
  );

  const selectedServer =
    view.mode === 'detail' ? (servers.find(s => s.server_id === view.serverId) ?? null) : null;
  const selectedConnStatus =
    view.mode === 'detail' ? statuses.find(s => s.server_id === view.serverId) : undefined;

  // One installed row per service; raw `servers` is kept for server_id-keyed
  // detail/status lookups. Memoized so the 5s status poll doesn't rebuild +
  // refilter the list.
  const filteredInstalled = useMemo(() => {
    const rows = dedupeInstalledByQualifiedName(servers);
    const q = searchQuery.trim().toLowerCase();
    if (!q) return rows;
    return rows.filter(
      s =>
        s.display_name.toLowerCase().includes(q) ||
        s.qualified_name.toLowerCase().includes(q) ||
        (s.description ?? '').toLowerCase().includes(q)
    );
  }, [servers, searchQuery]);

  const installedNames = useMemo(
    () => new Set(servers.map(s => s.qualified_name)),
    [servers]
  );

  const statusMap = new Map(statuses.map(s => [s.server_id, s]));

  if (loading) {
    return (
      <div className="py-10 text-center text-sm text-content-faint">{t('mcp.tab.loading')}</div>
    );
  }

  // Detail view — a server's own page, reached from the rows.
  if (view.mode === 'detail' && selectedServer) {
    return (
      <div className="space-y-3">
        <Button
          variant="tertiary"
          size="xs"
          onClick={() => setView({ mode: 'home' })}
          leadingIcon={
            <svg
              className="w-3.5 h-3.5"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
            </svg>
          }>
          {t('mcp.tab.back')}
        </Button>
        <InstalledServerDetail
          server={selectedServer}
          connStatus={selectedConnStatus}
          onUninstalled={serverId => void handleUninstalled(serverId)}
          onEnabledChange={(serverId, enabled) => void handleEnabledChange(serverId, enabled)}
        />
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <ChipTabs<McpPageTab>
        ariaLabel={t('mcp.tab.tablistAria')}
        testIdPrefix="mcp-page-tab"
        value={tab}
        onChange={setTab}
        items={[
          {
            id: 'servers',
            label: t('mcp.tab.section.servers').replace(
              '{count}',
              String(dedupeInstalledByQualifiedName(servers).length)
            ),
          },
          { id: 'json', label: t('mcp.tab.section.json') },
          { id: 'registry', label: t('mcp.tab.section.registry') },
        ]}
      />

      {tab === 'servers' && (
        <div className="space-y-3" data-testid="mcp-servers-section">
          <TextField
            type="search"
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            placeholder={t('mcp.installed.search.placeholder')}
            aria-label={t('mcp.installed.search.inputAria')}
          />

          {loadError && (
            <div className="rounded-lg border border-coral-200 dark:border-coral-500/30 bg-coral-50 dark:bg-coral-500/10 px-3 py-2 text-xs text-coral-700 dark:text-coral-300">
              {loadError}
            </div>
          )}

          {/* Connection health + bulk lifecycle actions. Only meaningful once
              servers are declared; reads the polled statuses — no extra
              fetches. */}
          {statuses.length > 0 && (
            <McpConnectionHealthToolbar
              statuses={statuses}
              onReconnect={handleReconnectAll}
              onDisconnect={handleDisconnectAll}
            />
          )}

          <Table className="min-w-[640px] rounded-lg border border-line">
            <TableHeader>
              <TableRow className="bg-surface-muted">
                <TableHead>{t('mcp.tab.column.name')}</TableHead>
                <TableHead className="hidden sm:table-cell">{t('mcp.tab.column.dial')}</TableHead>
                <TableHead className="hidden w-36 sm:table-cell">
                  {t('mcp.tab.column.author')}
                </TableHead>
                <TableHead className="w-28 text-right">{t('mcp.tab.column.action')}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filteredInstalled.map(server => {
                const status: ServerStatus =
                  statusMap.get(server.server_id)?.status ?? 'disconnected';
                return (
                  <TableRow
                    key={`installed-${server.server_id}`}
                    className="cursor-pointer"
                    tabIndex={0}
                    role="button"
                    data-testid="mcp-installed-row"
                    aria-label={t('mcp.tab.aria.viewDetails').replace(
                      '{name}',
                      server.display_name
                    )}
                    onClick={() => handleSelectServer(server.server_id)}
                    onKeyDown={e => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        handleSelectServer(server.server_id);
                      }
                    }}>
                    <TableCell>
                      <div className="flex items-center gap-2.5">
                        <span
                          className={`w-2 h-2 rounded-full shrink-0 ${STATUS_DOT[status]}`}
                          title={status}
                        />
                        <div className="min-w-0">
                          <span className="font-medium text-content truncate block">
                            {server.display_name}
                          </span>
                          {server.description && (
                            <span className="text-xs text-content-faint line-clamp-4 block">
                              {server.description}
                            </span>
                          )}
                        </div>
                      </div>
                    </TableCell>
                    <TableCell className="hidden sm:table-cell">
                      <span className="text-[11px] font-mono text-content-muted truncate block max-w-64">
                        {dialOf(server)}
                      </span>
                    </TableCell>
                    <TableCell className="hidden sm:table-cell">
                      <span className="text-xs text-content-muted truncate block">
                        {deriveAuthor(server.qualified_name) ?? '—'}
                      </span>
                    </TableCell>
                    <TableCell className="text-right">
                      <span className="text-xs text-primary-600 dark:text-primary-400 font-medium">
                        {t('mcp.tab.action.manage')}
                      </span>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>

          {filteredInstalled.length === 0 && (
            <div
              data-testid="mcp-installed-empty"
              className="rounded-b-lg border border-t-0 border-line py-8 text-center text-sm text-content-faint space-y-2">
              <p>
                {searchQuery
                  ? t('mcp.installed.search.noMatches').replace('{query}', searchQuery)
                  : t('mcp.installed.empty')}
              </p>
              {!searchQuery && (
                <p className="flex items-center justify-center gap-3">
                  <Button variant="tertiary" size="xs" onClick={() => setTab('json')}>
                    {t('mcp.installed.emptyAddInJson')}
                  </Button>
                  <Button variant="tertiary" size="xs" onClick={() => setTab('registry')}>
                    {t('mcp.installed.emptyBrowseRegistry')}
                  </Button>
                </p>
              )}
            </div>
          )}
        </div>
      )}

      {tab === 'json' && <McpJsonEditor onSaved={() => void handleDocumentSaved()} />}

      {tab === 'registry' && <McpRegistryBrowser installedNames={installedNames} />}
    </div>
  );
};

export default McpServersTab;
