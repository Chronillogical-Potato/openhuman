import debug from 'debug';
import {
  ChevronRight,
  Download,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Sparkles,
  Trash2,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { LuLibrary, LuSparkles } from 'react-icons/lu';
import { useNavigate } from 'react-router-dom';

import { useT } from '../../lib/i18n/I18nContext';
import { type CatalogEntry, skillRegistryApi } from '../../services/api/skillRegistryApi';
import {
  type InstallWorkflowFromUrlResult,
  skillsApi,
  type WorkflowSummary,
} from '../../services/api/skillsApi';
import McpIconButton from '../channels/mcp/McpIconButton';
import EmptyStateCard from '../EmptyStateCard';
import { Badge, DataTableFilterMenu, ModalShell } from '../ui';
import Button from '../ui/Button';
import Card from '../ui/Card';
import TextField from '../ui/TextField';
import CreateSkillModal from './CreateSkillModal';
import InstallSkillDialog from './InstallSkillDialog';
import UninstallSkillConfirmDialog from './UninstallSkillConfirmDialog';

const log = debug('skills:explorer-tab');
const CATALOG_PAGE_SIZE = 60;
const SEARCH_DEBOUNCE_MS = 300;

function slugifyInstallKey(value: string | null | undefined): string | null {
  const raw = value?.trim();
  if (!raw) return null;

  let out = '';
  let lastDash = false;
  for (const ch of raw) {
    if (/[a-z0-9]/i.test(ch)) {
      out += ch.toLowerCase();
      lastDash = false;
    } else if (!lastDash && out.length > 0) {
      out += '-';
      lastDash = true;
    }
  }
  return out.replace(/-+$/, '') || null;
}

function lastPathSegment(value: string | null | undefined): string | null {
  const raw = value?.trim();
  if (!raw) return null;
  const parts = raw.split(/[/:#?]+/).filter(Boolean);
  return parts.at(-1) ?? null;
}

function parentPathSegment(value: string | null | undefined): string | null {
  const raw = value?.trim();
  if (!raw) return null;
  const parts = raw.split(/[\\/]+/).filter(Boolean);
  return parts.length >= 2 ? (parts.at(-2) ?? null) : null;
}

function catalogInstallKeys(entry: CatalogEntry): string[] {
  return [
    slugifyInstallKey(entry.id),
    slugifyInstallKey(lastPathSegment(entry.id)),
    slugifyInstallKey(parentPathSegment(entry.docs_path)),
    slugifyInstallKey(parentPathSegment(entry.download_url)),
  ].filter((key): key is string => Boolean(key));
}

function workflowInstallKeys(skill: WorkflowSummary): string[] {
  return [slugifyInstallKey(skill.id), slugifyInstallKey(parentPathSegment(skill.location))].filter(
    (key): key is string => Boolean(key)
  );
}

function isCatalogEntryInstalled(entry: CatalogEntry, installedKeys: Set<string>): boolean {
  return catalogInstallKeys(entry).some(key => installedKeys.has(key));
}

/**
 * Source tone table. Six sources, four themeable ramps — so the hue encodes the
 * distinction a reader acts on (where does this skill come from: shipped with
 * the app, or fetched from a remote catalogue) rather than naming each
 * catalogue twice. The badge already prints the catalogue's name, so the four
 * remote rows fall through to the shared neutral tone instead of reaching for
 * unthemeable ramps. See `gitbooks/developing/theming.md`.
 */
const BADGE_NEUTRAL_TONE = 'bg-surface-muted text-content-secondary border-line';

function SourceBadge({ source }: { source: string }) {
  const SOURCE_COLORS: Record<string, string> = {
    'built-in':
      'bg-sage-50 text-sage-700 border-sage-200 dark:bg-sage-500/10 dark:text-sage-300 dark:border-sage-500/30',
    optional:
      'bg-primary-50 text-primary-700 border-primary-200 dark:bg-primary-500/10 dark:text-primary-300 dark:border-primary-500/30',
  };
  const colors = SOURCE_COLORS[source] ?? BADGE_NEUTRAL_TONE;
  return (
    <span
      className={`inline-flex items-center rounded-full border px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wider ${colors}`}>
      {source}
    </span>
  );
}

/**
 * Format tone table. Three distinct tones for five formats, so it fits inside
 * the four themeable ramps with no collision and every distinction survives:
 * the Hermes family on `primary`, the ClawHub family on `sage`, and `legacy`
 * on `amber` because it is the one row that means "deprecated".
 */
const FORMAT_TONE = {
  hermes:
    'bg-primary-50 text-primary-700 border-primary-200 dark:bg-primary-500/10 dark:text-primary-300 dark:border-primary-500/30',
  clawhub:
    'bg-sage-50 text-sage-700 border-sage-200 dark:bg-sage-500/10 dark:text-sage-300 dark:border-sage-500/30',
  legacy:
    'bg-amber-50 text-amber-700 border-amber-200 dark:bg-amber-500/10 dark:text-amber-300 dark:border-amber-500/30',
} as const;

function SkillFormatBadge({ format }: { format: string }) {
  const lower = format.toLowerCase();
  const FORMAT_MAP: Record<string, { label: string; colors: string }> = {
    hermes: { label: 'Hermes', colors: FORMAT_TONE.hermes },
    agentskills: { label: 'AgentSkills', colors: FORMAT_TONE.hermes },
    openclaw: { label: 'OpenClaw', colors: FORMAT_TONE.clawhub },
    clawhub: { label: 'ClawHub', colors: FORMAT_TONE.clawhub },
    legacy: { label: 'Legacy', colors: FORMAT_TONE.legacy },
  };
  const entry = FORMAT_MAP[lower] ?? {
    label: format || 'Skill',
    colors: BADGE_NEUTRAL_TONE,
  };
  return (
    <span
      className={`inline-flex items-center rounded-full border px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wider ${entry.colors}`}>
      {entry.label}
    </span>
  );
}

function SkillScopeBadge({ scope }: { scope: string }) {
  const { t } = useT();
  const label =
    scope === 'user'
      ? t('skills.explorer.scopeUser')
      : scope === 'project'
        ? t('skills.explorer.scopeProject')
        : t('skills.explorer.scopeLegacy');
  return (
    <span className="inline-flex items-center rounded-full border border-line bg-surface-muted px-1.5 py-0.5 text-[9px] font-medium text-content-muted">
      {label}
    </span>
  );
}

interface SkillTileProps {
  skill: WorkflowSummary;
  onUninstall: () => void;
  onClick: () => void;
  onRun: () => void;
  onEdit: () => void;
}

/**
 * One installed skill: an identity on the left (name, format, scope, version,
 * description, tags) and its controls as icons on the right — run, edit,
 * remove — the same row the MCP page uses. The name opens the detail dialog.
 */
function InstalledSkillRow({ skill, onUninstall, onClick, onRun, onEdit }: SkillTileProps) {
  const { t } = useT();
  const editable = skill.scope === 'user';
  return (
    <li
      data-testid={`skill-explorer-tile-${skill.id}`}
      className="space-y-2 py-3 first:pt-0 last:pb-0">
      <div className="flex items-start gap-3">
        <span className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-md border border-line bg-surface-muted">
          <Sparkles className="size-4 text-content-muted" aria-hidden="true" />
        </span>
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              data-testid={`skill-open-${skill.id}`}
              aria-label={t('skills.rows.open').replace('{name}', skill.name)}
              onClick={onClick}
              className="inline-flex cursor-pointer items-center gap-0.5 rounded-sm text-sm font-medium text-content transition-opacity hover:opacity-80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500">
              {skill.name}
              <ChevronRight className="size-3.5 text-content-muted" aria-hidden="true" />
            </button>
            <SkillFormatBadge format={skill.sourceFormat} />
            <SkillScopeBadge scope={skill.scope} />
            {skill.version && (
              <span className="text-[10px] font-mono text-content-faint">v{skill.version}</span>
            )}
          </div>
          <p className="line-clamp-2 text-xs text-content-muted">
            {skill.description || t('skills.explorer.noDescription')}
          </p>
          {(skill.tags.length > 0 || skill.warnings.length > 0) && (
            <div className="flex flex-wrap items-center gap-1">
              {skill.tags.map(tag => (
                <Badge key={tag} variant="neutral">
                  {tag}
                </Badge>
              ))}
              {skill.warnings.map(warning => (
                <span key={warning} className="text-[10px] text-amber-700 dark:text-amber-300">
                  {warning}
                </span>
              ))}
            </div>
          )}
        </div>
        <span className="flex shrink-0 items-center gap-0.5">
          <McpIconButton
            label={t('skills.rows.run').replace('{name}', skill.name)}
            icon={Play}
            tone="primary"
            testId={`skill-run-${skill.id}`}
            onClick={onRun}
          />
          {editable && (
            <McpIconButton
              label={t('skills.rows.edit').replace('{name}', skill.name)}
              icon={Pencil}
              testId={`skill-edit-${skill.id}`}
              onClick={onEdit}
            />
          )}
          {editable ? (
            <McpIconButton
              label={t('skills.rows.remove').replace('{name}', skill.name)}
              icon={Trash2}
              tone="destructive"
              testId={`skill-uninstall-${skill.id}`}
              onClick={onUninstall}
            />
          ) : (
            <Badge variant="neutral">{t('skills.explorer.installed')}</Badge>
          )}
        </span>
      </div>
    </li>
  );
}

interface CatalogTileProps {
  entry: CatalogEntry;
  installed: boolean;
  installing: boolean;
  onInstall: () => void;
  onClick: () => void;
}

interface SkillDetailDialogProps {
  entry: CatalogEntry | null;
  skill: WorkflowSummary | null;
  installed: boolean;
  onClose: () => void;
  onInstall?: () => void;
  installing?: boolean;
}

function CatalogRow({ entry, installed, installing, onInstall, onClick }: CatalogTileProps) {
  const { t } = useT();
  return (
    <li data-testid={`registry-tile-${entry.id}`} className="space-y-2 py-3 first:pt-0 last:pb-0">
      <div className="flex items-start gap-3">
        <span className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-md border border-line bg-surface-muted text-xs font-semibold text-content-muted">
          {entry.name.charAt(0).toUpperCase()}
        </span>
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              data-testid={`registry-open-${entry.id}`}
              aria-label={t('skills.rows.open').replace('{name}', entry.name)}
              onClick={onClick}
              className="inline-flex cursor-pointer items-center gap-0.5 rounded-sm text-sm font-medium text-content transition-opacity hover:opacity-80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500">
              {entry.name}
              <ChevronRight className="size-3.5 text-content-muted" aria-hidden="true" />
            </button>
            <SourceBadge source={entry.source} />
            {installed && <Badge variant="success">{t('skills.explorer.installed')}</Badge>}
          </div>
          <p className="line-clamp-2 text-xs text-content-muted">{entry.description}</p>
        </div>
        <span className="flex shrink-0 items-center">
          {installed ? null : !entry.download_url ? (
            <Badge
              variant="neutral"
              title={t('skills.explorer.notInstallableHint')}
              data-testid={`registry-not-installable-${entry.id}`}>
              {t('skills.explorer.notInstallable')}
            </Badge>
          ) : (
            <Button
              variant="secondary"
              size="sm"
              data-testid={`registry-install-${entry.id}`}
              disabled={installing}
              leadingIcon={<Download className="size-3.5" aria-hidden="true" />}
              onClick={onInstall}>
              {installing ? t('skills.explorer.installing') : t('skills.explorer.install')}
            </Button>
          )}
        </span>
      </div>
    </li>
  );
}

function SkillDetailDialog({
  entry,
  skill,
  installed,
  onClose,
  onInstall,
  installing,
}: SkillDetailDialogProps) {
  const { t } = useT();
  const name = entry?.name ?? skill?.name ?? '';
  const description = entry?.description ?? skill?.description ?? '';
  const tags = entry?.tags ?? skill?.tags ?? [];
  const version = entry?.version ?? skill?.version ?? '';
  const author = entry?.author ?? '';
  const source = entry?.source ?? '';
  const category = entry?.category ?? '';
  const downloadUrl = entry?.download_url ?? '';
  const license = entry?.license ?? '';

  return (
    <ModalShell
      onClose={onClose}
      titleId="skill-detail-title"
      maxWidthClassName="max-w-lg"
      contentClassName="p-5 space-y-4"
      title={
        <span className="flex items-center gap-2">
          <span className="truncate">{name}</span>
          {installed && (
            <span className="shrink-0 rounded-full border border-sage-200 dark:border-sage-500/30 bg-sage-50 dark:bg-sage-500/10 px-2 py-0.5 text-[10px] font-medium text-sage-700 dark:text-sage-300">
              {t('skills.explorer.installed')}
            </span>
          )}
        </span>
      }
      subtitle={
        <span className="mt-1.5 flex items-center gap-1.5">
          {source && <SourceBadge source={source} />}
          {category && (
            <span className="inline-flex items-center rounded-full border border-line bg-surface-muted px-1.5 py-0.5 text-[9px] font-medium text-content-muted">
              {category}
            </span>
          )}
        </span>
      }
      footer={
        !installed && onInstall ? (
          <div className="flex justify-end">
            {downloadUrl ? (
              <Button variant="secondary" size="sm" disabled={installing} onClick={onInstall}>
                {installing ? t('skills.explorer.installing') : t('skills.explorer.install')}
              </Button>
            ) : (
              <p className="text-xs text-content-muted">
                {t('skills.explorer.notInstallableHint')}
              </p>
            )}
          </div>
        ) : undefined
      }>
      <>
        {description && (
          <div>
            <h3 className="text-[11px] font-semibold uppercase tracking-wider text-content-faint mb-1">
              {t('skills.detail.description')}
            </h3>
            <p className="text-sm text-content-secondary leading-relaxed whitespace-pre-wrap">
              {description}
            </p>
          </div>
        )}

        <div className="flex flex-wrap gap-x-6 gap-y-2">
          {version && (
            <div>
              <span className="text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                {t('skills.detail.version')}
              </span>
              <p className="text-xs font-mono text-content-secondary">v{version}</p>
            </div>
          )}
          {author && (
            <div>
              <span className="text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                {t('skills.detail.author')}
              </span>
              <p className="text-xs text-content-secondary">{author}</p>
            </div>
          )}
          {license && (
            <div>
              <span className="text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                {t('skills.detail.license')}
              </span>
              <p className="text-xs text-content-secondary">{license}</p>
            </div>
          )}
        </div>

        {tags.length > 0 && (
          <div>
            <h3 className="text-[11px] font-semibold uppercase tracking-wider text-content-faint mb-1.5">
              {t('skills.detail.tags')}
            </h3>
            <div className="flex flex-wrap gap-1.5">
              {tags.map(tag => (
                <span
                  key={tag}
                  className="rounded-full bg-surface-subtle px-2 py-0.5 text-[10px] font-medium text-content-secondary">
                  {tag}
                </span>
              ))}
            </div>
          </div>
        )}

        {downloadUrl && (
          <div>
            <h3 className="text-[11px] font-semibold uppercase tracking-wider text-content-faint mb-1">
              {t('skills.detail.source')}
            </h3>
            <p className="text-[11px] font-mono text-content-faint break-all">{downloadUrl}</p>
          </div>
        )}
      </>
    </ModalShell>
  );
}

export type ExplorerView = 'installed' | 'registry';

interface SkillsExplorerTabProps {
  onToast?: (toast: { type: 'success' | 'error'; title: string; message?: string }) => void;
  /** Which notation the page header picked. */
  view: ExplorerView;
}

export default function SkillsExplorerTab({ onToast, view }: SkillsExplorerTabProps) {
  const { t } = useT();
  const navigate = useNavigate();

  const [skills, setSkills] = useState<WorkflowSummary[]>([]);
  const [skillsLoading, setSkillsLoading] = useState(true);
  const [skillsError, setSkillsError] = useState<string | null>(null);

  const [catalogEntries, setCatalogEntries] = useState<CatalogEntry[]>([]);
  // How many catalog entries are currently revealed. We fetch the whole list
  // up front, then page through it client-side via the "Show more" control.
  const [visibleCount, setVisibleCount] = useState(CATALOG_PAGE_SIZE);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [catalogInitialized, setCatalogInitialized] = useState(false);
  const [installingId, setInstallingId] = useState<string | null>(null);
  // Catalog entry ids we just installed this session. The "installed" badge is
  // otherwise derived purely from `isCatalogEntryInstalled`, a heuristic that
  // maps a refetched installed skill (whose post-install id/location can differ
  // from the catalog entry) back to the catalog card. When that mapping misses,
  // a successful install fell back to "Install" — the only signal was a fleeting
  // toast, so the card looked unchanged (#4150). Recording the installed entry
  // id here makes the card flip to "Installed" deterministically on success.
  const [installedEntryIds, setInstalledEntryIds] = useState<Set<string>>(new Set());

  const [sources, setSources] = useState<string[]>([]);
  const [activeSources, setActiveSources] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const [installDialogOpen, setInstallDialogOpen] = useState(false);
  // `null` closed, `undefined` creating, a skill editing.
  const [createOpen, setCreateOpen] = useState<WorkflowSummary | null | undefined>(null);
  const [uninstallTarget, setUninstallTarget] = useState<WorkflowSummary | null>(null);
  const [detailEntry, setDetailEntry] = useState<CatalogEntry | null>(null);
  const [detailSkill, setDetailSkill] = useState<WorkflowSummary | null>(null);

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Debounce search input
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      setDebouncedQuery(searchQuery);
    }, SEARCH_DEBOUNCE_MS);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [searchQuery]);

  const fetchSkills = useCallback(async () => {
    log('fetchSkills: start');
    setSkillsLoading(true);
    setSkillsError(null);
    try {
      // Include `skills/`-root installs (registry installs land there) so they
      // appear in the Installed tab and flip the catalog Install button.
      const result = await skillsApi.listWorkflows({ includeSkills: true });
      log('fetchSkills: count=%d', result.length);
      setSkills(result);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      log('fetchSkills: error=%s', msg);
      setSkillsError(msg);
    } finally {
      setSkillsLoading(false);
    }
  }, []);

  // Compute the active source filter for RPC calls.
  // Only apply when the user has deselected at least one source.
  const activeSourceFilter = useMemo(() => {
    if (activeSources.size === 0 || activeSources.size >= sources.length) return undefined;
    // If exactly one source is active, pass it as the filter
    if (activeSources.size === 1) return [...activeSources][0];
    return undefined;
  }, [activeSources, sources.length]);

  // Fetch catalog via RPC search (handles both browse and search).
  // When query is empty and no source filter, uses browse; otherwise uses search.
  const fetchCatalog = useCallback(
    async (query: string, sourceFilter: string | undefined, forceRefresh: boolean) => {
      log('fetchCatalog: query=%s source=%s forceRefresh=%s', query, sourceFilter, forceRefresh);
      setCatalogLoading(true);
      setCatalogError(null);
      try {
        let entries: CatalogEntry[];
        if (!query && !sourceFilter && !forceRefresh) {
          entries = await skillRegistryApi.browse(false);
        } else if (!query && !sourceFilter && forceRefresh) {
          entries = await skillRegistryApi.browse(true);
        } else {
          entries = await skillRegistryApi.search(query || '', sourceFilter);
        }
        log('fetchCatalog: total=%d', entries.length);
        // Keep the full list so "Show more" can page through it without another
        // RPC; only a window of it is rendered (see displayedCatalog).
        setCatalogEntries(entries);
        setVisibleCount(CATALOG_PAGE_SIZE);
        setCatalogInitialized(true);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        log('fetchCatalog: error=%s', msg);
        setCatalogError(msg);
      } finally {
        setCatalogLoading(false);
      }
    },
    []
  );

  useEffect(() => {
    void fetchSkills();
    skillRegistryApi
      .sources()
      .then(s => {
        setSources(s);
        setActiveSources(new Set(s));
      })
      .catch(() => {});
  }, [fetchSkills]);

  // Trigger catalog search when debounced query or source filter changes
  useEffect(() => {
    if (view === 'registry') {
      void fetchCatalog(debouncedQuery, activeSourceFilter, false);
    }
  }, [view, debouncedQuery, activeSourceFilter, fetchCatalog]);

  const installedKeys = useMemo(
    () => new Set(skills.flatMap(skill => workflowInstallKeys(skill))),
    [skills]
  );

  // A catalog entry counts as installed if the refetched installed list maps
  // back to it (`isCatalogEntryInstalled`) OR we installed it this session. The
  // latter guarantees the card reflects a successful install even when the
  // heuristic key-match misses (#4150).
  const entryInstalled = useCallback(
    (entry: CatalogEntry): boolean =>
      installedEntryIds.has(entry.id) || isCatalogEntryInstalled(entry, installedKeys),
    [installedEntryIds, installedKeys]
  );

  const filteredSkills = useMemo(() => {
    const q = searchQuery.toLowerCase().trim();
    if (!q) return skills;
    return skills.filter(
      s =>
        s.name.toLowerCase().includes(q) ||
        s.description.toLowerCase().includes(q) ||
        s.tags.some(tag => tag.toLowerCase().includes(q)) ||
        s.sourceFormat.toLowerCase().includes(q)
    );
  }, [skills, searchQuery]);

  const sortedSkills = useMemo(() => {
    return [...filteredSkills].sort((a, b) => {
      if (a.sourceFormat === 'hermes' && b.sourceFormat !== 'hermes') return -1;
      if (a.sourceFormat !== 'hermes' && b.sourceFormat === 'hermes') return 1;
      return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
    });
  }, [filteredSkills]);

  // When multiple sources are active (but not all), do client-side filtering
  // on the already-fetched results since the RPC only supports single source filter.
  const filteredCatalog = useMemo(() => {
    if (
      activeSources.size === 0 ||
      activeSources.size >= sources.length ||
      activeSources.size === 1
    ) {
      return catalogEntries;
    }
    return catalogEntries.filter(e => activeSources.has(e.source));
  }, [catalogEntries, activeSources, sources.length]);

  // Client-side pagination window: we already hold the full fetched list, so
  // "Show more" reveals the next page instantly with no extra RPC.
  const displayedCatalog = useMemo(
    () => filteredCatalog.slice(0, visibleCount),
    [filteredCatalog, visibleCount]
  );

  const handleInstalled = useCallback(
    (result: InstallWorkflowFromUrlResult) => {
      log('handleInstalled: newSkills=%d', result.newWorkflows.length);
      void fetchSkills();
      if (result.newWorkflows.length > 0) {
        onToast?.({
          type: 'success',
          title: t('skills.install.installComplete'),
          message: t('skills.install.successDiscovered').replace(
            '{count}',
            String(result.newWorkflows.length)
          ),
        });
      }
    },
    [fetchSkills, onToast, t]
  );

  const handleUninstalled = useCallback(() => {
    log('handleUninstalled');
    void fetchSkills();
    onToast?.({ type: 'success', title: t('skills.explorer.uninstallSuccess') });
  }, [fetchSkills, onToast, t]);

  const handleRegistryInstall = useCallback(
    async (entry: CatalogEntry) => {
      log('handleRegistryInstall: id=%s source=%s', entry.id, entry.source);
      setInstallingId(entry.id);
      try {
        const result = await skillRegistryApi.install(entry.id);
        // Authoritatively mark this entry installed so the card flips to
        // "Installed" on success regardless of whether the refetched list maps
        // back to it via the install-key heuristic (#4150).
        setInstalledEntryIds(prev => {
          const next = new Set(prev);
          next.add(entry.id);
          return next;
        });
        // Await the refetch so `installedKeys` is fresh before the button
        // re-renders — otherwise it briefly flips back to "Install" between
        // clearing the installing state and the list updating. `fetchSkills`
        // swallows its own errors, so this never throws into the catch below.
        await fetchSkills();
        onToast?.({
          type: 'success',
          title: t('skills.install.installComplete'),
          message: `Installed ${entry.name}${result.newSkills.length > 0 ? ` (${result.newSkills.join(', ')})` : ''}`,
        });
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        log('handleRegistryInstall: error=%s', msg);
        onToast?.({ type: 'error', title: t('skills.install.errors.genericTitle'), message: msg });
      } finally {
        setInstallingId(null);
      }
    },
    [fetchSkills, onToast, t]
  );

  const loading = view === 'installed' ? skillsLoading : catalogLoading;
  const error = view === 'installed' ? skillsError : catalogError;

  const runSkill = (skill: WorkflowSummary) =>
    navigate(`/workflows/run?workflow=${encodeURIComponent(skill.id)}&lock=1`);

  const errorNode =
    !loading && error ? (
      <div className="rounded-xl border border-coral-200 bg-coral-50 p-3 dark:border-coral-500/30 dark:bg-coral-500/10">
        <p className="text-xs font-medium text-coral-700 dark:text-coral-300">{error}</p>
        <Button
          variant="secondary"
          tone="danger"
          size="xs"
          onClick={() =>
            void (view === 'installed'
              ? fetchSkills()
              : fetchCatalog(debouncedQuery, activeSourceFilter, true))
          }
          className="mt-2">
          {t('common.retry')}
        </Button>
      </div>
    ) : null;

  const search = (
    <TextField
      type="search"
      value={searchQuery}
      onChange={e => setSearchQuery(e.target.value)}
      placeholder={t('skills.explorer.searchPlaceholder')}
      aria-label={t('skills.explorer.title')}
      data-testid="skill-search-input"
      className="min-w-48 flex-1"
    />
  );

  const installedEmpty =
    // A search that matched nothing is not the same as having no skills — the
    // second offers an install CTA, the first would be nonsense.
    skills.length > 0 ? (
      <p className="py-4 text-center text-xs text-content-faint">{t('skills.noResults')}</p>
    ) : (
      <EmptyStateCard
        className="py-6"
        icon={<LuSparkles className="h-7 w-7 text-primary-500" strokeWidth={1.5} />}
        title={t('skills.explorer.emptyTitle')}
        description={t('skills.explorer.emptyDescription')}
        actionLabel={t('skills.explorer.emptyCta')}
        onAction={() => setInstallDialogOpen(true)}
      />
    );

  const registryEmpty = catalogInitialized ? (
    <EmptyStateCard
      className="py-6"
      icon={<LuLibrary className="h-7 w-7 text-primary-500" strokeWidth={1.5} />}
      title={debouncedQuery ? t('skills.noResults') : t('skills.explorer.registryEmptyTitle')}
      description={debouncedQuery ? '' : t('skills.explorer.registryEmptyDescription')}
      actionLabel={debouncedQuery ? undefined : t('skills.explorer.refreshRegistry')}
      onAction={debouncedQuery ? undefined : () => void fetchCatalog('', undefined, true)}
    />
  ) : null;

  const dialogs = (
    <>
      {installDialogOpen && (
        <InstallSkillDialog
          onClose={() => setInstallDialogOpen(false)}
          onInstalled={handleInstalled}
        />
      )}

      {createOpen !== null && (
        <CreateSkillModal
          editing={createOpen ?? undefined}
          onClose={() => setCreateOpen(null)}
          onCreated={() => {
            setCreateOpen(null);
            void fetchSkills();
          }}
        />
      )}

      {uninstallTarget && (
        <UninstallSkillConfirmDialog
          skill={uninstallTarget}
          onClose={() => setUninstallTarget(null)}
          onUninstalled={handleUninstalled}
        />
      )}

      {(detailEntry || detailSkill) && (
        <SkillDetailDialog
          entry={detailEntry}
          skill={detailSkill}
          installed={detailEntry ? entryInstalled(detailEntry) : true}
          onClose={() => {
            setDetailEntry(null);
            setDetailSkill(null);
          }}
          onInstall={
            detailEntry && !entryInstalled(detailEntry)
              ? () => {
                  void handleRegistryInstall(detailEntry);
                  setDetailEntry(null);
                }
              : undefined
          }
          installing={detailEntry ? installingId === detailEntry.id : false}
        />
      )}
    </>
  );

  if (view === 'installed') {
    return (
      <section className="space-y-3 animate-fade-up" data-testid="skills-installed-section">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h2 className="text-xs font-medium uppercase tracking-wide text-content-muted">
            {t('skills.rows.installedTitle')}
          </h2>
          <div className="flex items-center gap-2">
            <Button
              variant="secondary"
              size="sm"
              data-testid="skill-install-from-url-btn"
              onClick={() => setInstallDialogOpen(true)}
              leadingIcon={<Download className="size-4" aria-hidden="true" />}>
              {t('skills.explorer.installFromUrl')}
            </Button>
            <Button
              variant="primary"
              size="sm"
              data-testid="skill-new-btn"
              onClick={() => setCreateOpen(undefined)}
              leadingIcon={<Plus className="size-4" aria-hidden="true" />}>
              {t('skills.explorer.newSkill')}
            </Button>
          </div>
        </div>
        <p className="text-sm text-content-muted">{t('skills.rows.installedIntro')}</p>
        {search}
        {errorNode}
        <Card padded divided={false}>
          {loading ? (
            <p className="text-xs text-content-muted">{t('common.loading')}</p>
          ) : sortedSkills.length === 0 ? (
            installedEmpty
          ) : (
            <ul className="divide-y divide-line-subtle">
              {sortedSkills.map(skill => (
                <InstalledSkillRow
                  key={skill.id}
                  skill={skill}
                  onClick={() => setDetailSkill(skill)}
                  onRun={() => runSkill(skill)}
                  onEdit={() => setCreateOpen(skill)}
                  onUninstall={() => setUninstallTarget(skill)}
                />
              ))}
            </ul>
          )}
        </Card>
        {dialogs}
      </section>
    );
  }

  return (
    <section className="space-y-3 animate-fade-up" data-testid="skills-registry-section">
      <h2 className="text-xs font-medium uppercase tracking-wide text-content-muted">
        {t('skills.rows.registryTitle')}
      </h2>
      <p className="text-sm text-content-muted">{t('skills.rows.registryIntro')}</p>
      <div className="flex flex-wrap items-center gap-2">
        {search}
        {sources.length > 0 && (
          <DataTableFilterMenu
            filter={{
              id: 'source',
              label: t('common.filter'),
              ariaLabel: t('skills.explorer.sourceFilterAria'),
              testId: 'skill-source-filter',
              options: sources.map(source => ({ value: source })),
              selected: activeSources,
              onChange: setActiveSources,
            }}
          />
        )}
        <Button
          iconOnly
          variant="secondary"
          size="md"
          onClick={() => void fetchCatalog(debouncedQuery, activeSourceFilter, true)}
          disabled={catalogLoading}
          title={t('skills.explorer.refreshRegistry')}
          aria-label={t('skills.explorer.refreshRegistry')}
          className="shrink-0 text-content-muted">
          <RefreshCw className={`size-4 ${catalogLoading ? 'animate-spin' : ''}`} />
        </Button>
      </div>
      {errorNode}
      <Card padded divided={false}>
        {loading && displayedCatalog.length === 0 ? (
          <p className="text-xs text-content-muted">{t('common.loading')}</p>
        ) : displayedCatalog.length === 0 ? (
          registryEmpty
        ) : (
          <>
            <ul className="divide-y divide-line-subtle">
              {displayedCatalog.map(entry => (
                <CatalogRow
                  key={`${entry.source}-${entry.id}`}
                  entry={entry}
                  installed={entryInstalled(entry)}
                  installing={installingId === entry.id}
                  onClick={() => setDetailEntry(entry)}
                  onInstall={() => void handleRegistryInstall(entry)}
                />
              ))}
            </ul>
            {filteredCatalog.length > displayedCatalog.length && (
              <div className="mt-3 flex flex-col items-center gap-1 border-t border-line-subtle pt-3">
                <Button
                  variant="tertiary"
                  size="xs"
                  data-testid="registry-show-more"
                  onClick={() => setVisibleCount(c => c + CATALOG_PAGE_SIZE)}
                  className="text-primary-600 hover:underline dark:text-primary-400">
                  {t('common.showMore')}
                </Button>
                <p className="text-[11px] text-content-faint">
                  {displayedCatalog.length.toLocaleString()} /{' '}
                  {filteredCatalog.length.toLocaleString()}
                </p>
              </div>
            )}
          </>
        )}
      </Card>
      {dialogs}
    </section>
  );
}
