/**
 * Servers — the user's declared MCP servers, a row each.
 *
 * The first tab of the MCP page. A row is an identity on the left (icon, name,
 * how it runs, its live status) and its controls as icons on the right:
 * connect or disconnect, enable or disable, remove. Labelled buttons pushed a
 * row four controls deep onto two lines, and the second line was always the
 * one carrying the command — the row's own information lost to its chrome.
 * Every icon keeps its sentence in a tooltip and in its accessible name; see
 * `McpIconButton`.
 *
 * The name opens the server's detail (credential form, tool list, playground).
 * Nothing here adds a server: that is the mcp.json tab's job, and the empty
 * state says so.
 */
import {
  AlertTriangle,
  Check,
  ChevronRight,
  Info,
  Pencil,
  Plug,
  Plus,
  Power,
  PowerOff,
  Server,
  Trash2,
  Unplug,
  Wrench,
} from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';

import { useT } from '../../../lib/i18n/I18nContext';
import { mcpClientsApi } from '../../../services/api/mcpClientsApi';
import Badge from '../../ui/Badge';
import Button from '../../ui/Button';
import Card from '../../ui/Card';
import { ConfirmDialog } from '../../ui/ConfirmDialog';
import ConnectAuthModal from './ConnectAuthModal';
import McpIconButton from './McpIconButton';
import McpServerForm from './McpServerForm';
import McpToolPlayground from './McpToolPlayground';
import type { ConnStatus, InstalledServer, McpTool, ServerStatus } from './types';

interface McpServerRowsProps {
  servers: InstalledServer[];
  statuses: ConnStatus[];
  /** Open a server's detail view. */
  onOpen: (serverId: string) => void;
  /** Re-read rows and statuses after a control changed something. */
  onChanged: () => Promise<void>;
  /** Jump to the mcp.json tab, from the empty state. */
  onAddInJson: () => void;
}

/** How a row is dialled, as one line: the hosted endpoint or the local command. */
const dialOf = (server: InstalledServer): string => {
  if (server.transport?.kind === 'http_remote') return server.transport.url;
  return [server.command, ...server.args].filter(Boolean).join(' ');
};

/** The status line's tone and icon. `null` for a plain disconnected row. */
const statusTone = (
  status: ServerStatus
): { className: string; Icon: typeof Check; key: string } | null => {
  switch (status) {
    case 'connected':
      return { className: 'text-sage-700 dark:text-sage-300', Icon: Check, key: 'connected' };
    case 'connecting':
      return { className: 'text-content-muted', Icon: Info, key: 'connecting' };
    case 'unauthorized':
      return {
        className: 'text-amber-700 dark:text-amber-300',
        Icon: AlertTriangle,
        key: 'unauthorized',
      };
    case 'error':
      return { className: 'text-coral-600 dark:text-coral-300', Icon: AlertTriangle, key: 'error' };
    default:
      return null;
  }
};

type ToolsState =
  | { kind: 'loading' }
  | { kind: 'error'; message: string }
  | { kind: 'ready'; tools: McpTool[] };

/**
 * The tools one connected server advertises, listed under its row, each with
 * a Try button that opens the execution playground. Read on demand — the row
 * carries a tool *count* from the status poll; the names and schemas are
 * fetched when the user asks for them.
 */
const McpRowTools = ({
  server,
  onTry,
}: {
  server: InstalledServer;
  onTry: (tool: McpTool) => void;
}) => {
  const { t } = useT();
  const [state, setState] = useState<ToolsState>({ kind: 'loading' });

  useEffect(() => {
    let live = true;
    mcpClientsApi
      .listTools(server.server_id)
      .then(tools => {
        if (live) setState({ kind: 'ready', tools });
      })
      .catch((err: unknown) => {
        if (live) {
          setState({
            kind: 'error',
            message: err instanceof Error ? err.message : t('mcp.rows.toolsFailed'),
          });
        }
      });
    return () => {
      live = false;
    };
  }, [server.server_id, t]);

  if (state.kind === 'loading') {
    return (
      <p className="text-xs text-content-muted" data-testid="mcp-row-tools-loading">
        {t('mcp.rows.toolsLoading')}
      </p>
    );
  }
  if (state.kind === 'error') {
    return <p className="text-xs text-coral-600 dark:text-coral-300">{state.message}</p>;
  }
  if (state.tools.length === 0) {
    return <p className="text-xs text-content-muted">{t('mcp.toolList.noTools')}</p>;
  }
  return (
    <ul className="space-y-1 rounded-md bg-surface-muted p-2" data-testid="mcp-row-tools">
      {state.tools.map(tool => (
        <li key={tool.name} className="flex items-start justify-between gap-2 text-xs">
          <span className="min-w-0">
            <span className="font-mono font-medium text-content">{tool.name}</span>
            {tool.description ? (
              <span className="text-content-muted"> — {tool.description}</span>
            ) : null}
          </span>
          <Button
            variant="tertiary"
            size="xs"
            onClick={() => onTry(tool)}
            aria-label={t('mcp.toolList.tryToolAria').replace('{name}', tool.name)}
            className="h-auto shrink-0 p-0 font-medium text-primary-600 hover:underline dark:text-primary-400">
            {t('mcp.toolList.tryTool')}
          </Button>
        </li>
      ))}
    </ul>
  );
};

const McpServerRows = ({
  servers,
  statuses,
  onOpen,
  onChanged,
  onAddInJson,
}: McpServerRowsProps) => {
  const { t } = useT();
  // The id of the row currently mutating. Every handler serialises on it, so
  // while one is in flight the controls on ALL rows disable, not just the busy
  // one: a guard that is invisible on the other rows accepts clicks and
  // silently does nothing. The active control still shows its own spinner.
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [connectFor, setConnectFor] = useState<InstalledServer | null>(null);
  const [removeFor, setRemoveFor] = useState<InstalledServer | null>(null);
  // `null` closed, `undefined` adding, a server editing.
  const [form, setForm] = useState<InstalledServer | null | undefined>(null);
  // The rows whose tool lists are open, and the tool staged in the playground.
  const [toolsOpen, setToolsOpen] = useState<Set<string>>(() => new Set());
  const [playground, setPlayground] = useState<{ server: InstalledServer; tool: McpTool } | null>(
    null
  );
  const toggleTools = (serverId: string) =>
    setToolsOpen(prev => {
      const next = new Set(prev);
      if (next.has(serverId)) next.delete(serverId);
      else next.add(serverId);
      return next;
    });

  // The form wrote the document; the rows re-read it. A server saved with
  // browser sign-in is then opened straight into the connect dialog, which
  // runs the sign-in — the form cannot, because it needs the server's id.
  const handleFormSaved = useCallback(
    async (name: string, { signIn }: { signIn: boolean }) => {
      setForm(null);
      await onChanged();
      if (!signIn) return;
      const fresh = await mcpClientsApi.installedList();
      const saved = fresh.find(s => s.qualified_name === name);
      if (saved) setConnectFor(saved);
    },
    [onChanged]
  );

  const run = useCallback(
    async (serverId: string, task: () => Promise<void>) => {
      if (busy) return;
      setBusy(serverId);
      setError(null);
      try {
        await task();
        await onChanged();
      } catch (err) {
        setError(err instanceof Error ? err.message : t('mcp.rows.opFailed'));
      } finally {
        setBusy(null);
      }
    },
    [busy, onChanged, t]
  );

  const statusMap = new Map(statuses.map(s => [s.server_id, s]));

  return (
    <section className="space-y-3" data-testid="mcp-servers-section">
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-xs font-medium uppercase tracking-wide text-content-muted">
          {t('mcp.rows.title')}
        </h2>
        <Button
          variant="primary"
          size="sm"
          leadingIcon={<Plus className="size-4" aria-hidden="true" />}
          onClick={() => setForm(undefined)}
          data-testid="mcp-add-server">
          {t('mcp.rows.add')}
        </Button>
      </div>
      <p className="text-sm text-content-muted">{t('mcp.rows.intro')}</p>

      {error && (
        <p
          role="alert"
          className="rounded-md border border-coral-500/30 bg-coral-500/10 px-3 py-2 text-xs text-coral-700 dark:text-coral-300">
          {error}
        </p>
      )}

      <Card padded divided={false}>
        {servers.length === 0 ? (
          <div className="space-y-2" data-testid="mcp-installed-empty">
            <p className="text-sm text-content-muted">{t('mcp.installed.empty')}</p>
            <p className="flex items-center gap-3">
              <Button
                variant="tertiary"
                size="xs"
                onClick={() => setForm(undefined)}
                className="h-auto p-0">
                {t('mcp.rows.add')}
              </Button>
              <Button variant="tertiary" size="xs" onClick={onAddInJson} className="h-auto p-0">
                {t('mcp.installed.emptyAddInJson')}
              </Button>
            </p>
          </div>
        ) : (
          <ul className="divide-y divide-line-subtle">
            {servers.map(server => {
              const conn = statusMap.get(server.server_id);
              const status: ServerStatus = conn?.status ?? 'disconnected';
              const tone = statusTone(status);
              const hosted = server.transport?.kind === 'http_remote';
              const connected = status === 'connected';
              const rowBusy = busy === server.server_id;
              return (
                <li
                  key={server.server_id}
                  data-testid="mcp-installed-row"
                  className="space-y-2 py-3 first:pt-0 last:pb-0">
                  <div className="flex items-start gap-3">
                    <span className="mt-0.5 flex size-8 shrink-0 items-center justify-center overflow-hidden rounded-md border border-line bg-surface-muted">
                      {server.icon_url ? (
                        <img src={server.icon_url} alt="" className="size-full object-cover" />
                      ) : (
                        <Server className="size-4 text-content-muted" aria-hidden="true" />
                      )}
                    </span>
                    <div className="min-w-0 flex-1 space-y-1">
                      <div className="flex flex-wrap items-center gap-2">
                        {/* The row's handle on its own detail view. A button on
                            the name rather than a trailing "Open": the row's
                            right edge is already several controls deep, and the
                            name is what a user points at when they want to know
                            what a server is. The chevron is not decoration —
                            hover styling alone makes a name that opens something
                            indistinguishable from one that does not. */}
                        <button
                          type="button"
                          data-testid="mcp-server-open"
                          aria-label={t('mcp.rows.open').replace('{name}', server.display_name)}
                          onClick={() => onOpen(server.server_id)}
                          className="inline-flex cursor-pointer items-center gap-0.5 rounded-sm text-sm font-medium text-content transition-opacity hover:opacity-80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500">
                          {server.display_name}
                          <ChevronRight
                            className="size-3.5 text-content-muted"
                            aria-hidden="true"
                          />
                        </button>
                        <Badge variant="neutral">
                          {t(hosted ? 'mcp.tab.transport.hosted' : 'mcp.tab.transport.local')}
                        </Badge>
                        {tone && (
                          <span
                            className={`inline-flex items-center gap-1 text-xs ${tone.className}`}
                            data-testid="mcp-row-status">
                            <tone.Icon className="size-3" aria-hidden="true" />
                            {t(
                              tone.key === 'unauthorized'
                                ? 'mcp.status.unauthorized'
                                : `channels.status.${tone.key}`
                            )}
                            {connected && conn && conn.tool_count > 0
                              ? ` · ${t(
                                  conn.tool_count === 1
                                    ? 'mcp.installed.toolSingular'
                                    : 'mcp.installed.toolPlural'
                                ).replace('{count}', String(conn.tool_count))}`
                              : null}
                          </span>
                        )}
                        {!server.enabled && (
                          <Badge variant="neutral" data-testid="mcp-disabled-badge">
                            {t('mcp.status.disabled')}
                          </Badge>
                        )}
                      </div>
                      <p className="truncate font-mono text-xs text-content-muted">
                        {dialOf(server)}
                      </p>
                    </div>
                    <span className="flex shrink-0 items-center gap-0.5">
                      {server.enabled && (
                        <McpIconButton
                          label={t(connected ? 'mcp.rows.disconnect' : 'mcp.rows.connect').replace(
                            '{name}',
                            server.display_name
                          )}
                          icon={connected ? Unplug : Plug}
                          tone={connected ? 'default' : 'primary'}
                          testId="mcp-lifecycle"
                          busy={rowBusy}
                          disabled={busy !== null}
                          onClick={() => {
                            if (connected) {
                              void run(server.server_id, async () => {
                                await mcpClientsApi.disconnect(server.server_id);
                              });
                            } else {
                              setConnectFor(server);
                            }
                          }}
                        />
                      )}
                      {connected && (
                        <McpIconButton
                          label={t(
                            toolsOpen.has(server.server_id)
                              ? 'mcp.rows.hideTools'
                              : 'mcp.rows.showTools'
                          ).replace('{name}', server.display_name)}
                          icon={Wrench}
                          testId="mcp-tools"
                          disabled={busy !== null}
                          onClick={() => toggleTools(server.server_id)}
                        />
                      )}
                      <McpIconButton
                        label={t(server.enabled ? 'mcp.rows.disable' : 'mcp.rows.enable').replace(
                          '{name}',
                          server.display_name
                        )}
                        icon={server.enabled ? Power : PowerOff}
                        testId="mcp-toggle"
                        busy={rowBusy}
                        disabled={busy !== null}
                        onClick={() =>
                          void run(server.server_id, async () => {
                            await mcpClientsApi.setEnabled(server.server_id, !server.enabled);
                          })
                        }
                      />
                      <McpIconButton
                        label={t('mcp.rows.edit').replace('{name}', server.display_name)}
                        icon={Pencil}
                        testId="mcp-edit"
                        disabled={busy !== null}
                        onClick={() => setForm(server)}
                      />
                      <McpIconButton
                        label={t('mcp.rows.remove').replace('{name}', server.display_name)}
                        icon={Trash2}
                        tone="destructive"
                        testId="mcp-remove"
                        disabled={busy !== null}
                        onClick={() => setRemoveFor(server)}
                      />
                    </span>
                  </div>
                  {conn?.last_error && status !== 'connected' && (
                    <p className="text-xs text-content-muted">{conn.last_error}</p>
                  )}
                  {connected && toolsOpen.has(server.server_id) && (
                    <McpRowTools server={server} onTry={tool => setPlayground({ server, tool })} />
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </Card>

      {playground && (
        <McpToolPlayground
          serverId={playground.server.server_id}
          tool={playground.tool}
          onClose={() => setPlayground(null)}
        />
      )}

      {form !== null && (
        <McpServerForm
          existing={form ?? undefined}
          onClose={() => setForm(null)}
          onSaved={(name, opts) => void handleFormSaved(name, opts)}
        />
      )}

      {connectFor && (
        <ConnectAuthModal
          server={connectFor}
          onClose={() => setConnectFor(null)}
          onConnected={() => {
            setConnectFor(null);
            void onChanged();
          }}
        />
      )}

      {removeFor && (
        <ConfirmDialog
          title={t('mcp.rows.removeTitle').replace('{name}', removeFor.display_name)}
          body={t('mcp.rows.removeBody')}
          confirmLabel={t('common.remove')}
          destructive
          busy={busy === removeFor.server_id}
          testId="mcp-remove-dialog"
          onCancel={() => setRemoveFor(null)}
          onConfirm={() => {
            const target = removeFor;
            void run(target.server_id, async () => {
              await mcpClientsApi.uninstall(target.server_id);
            }).then(() => setRemoveFor(null));
          }}
        />
      )}
    </section>
  );
};

export default McpServerRows;
