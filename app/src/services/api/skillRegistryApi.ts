import debug from 'debug';

import { callCoreRpc } from '../coreRpcClient';

const log = debug('skillRegistryApi');

const CATALOG_RPC_TIMEOUT_MS = 120_000;

const INSTALL_RPC_TIMEOUT_MS = 120_000;

/** How fresh the catalog behind a page is. */
export type RegistryFreshness = 'live' | 'cached' | 'local_fallback';

/** Stable registry error kinds (`RegistryErrorKind` in tinyskills). */
export type RegistryErrorKind =
  | 'timeout'
  | 'unavailable'
  | 'rate_limited'
  | 'too_large'
  | 'malformed'
  | 'not_found'
  | 'ambiguous'
  | 'upstream_ambiguous'
  | 'no_direct_download'
  | 'unsafe_url'
  | 'invalid_document'
  | 'unknown_registry'
  | 'store'
  | 'transport_contract'
  | 'transport';

export interface RegistryErrorSummary {
  kind: RegistryErrorKind;
  message: string;
  retry_after_secs?: number | null;
}

export interface CatalogEntry {
  id: string;
  name: string;
  description: string;
  source: string;
  category: string;
  author: string | null;
  version: string | null;
  tags: string[];
  platforms: string[];
  /** Empty when the entry has no direct SKILL.md download. */
  download_url: string;
  /** Human-facing page for the entry. */
  source_url?: string | null;
  docs_path: string | null;
  commands: string[];
  env_vars: string[];
  license: string | null;
  /** The registry the entry came from. */
  registry?: string;
  /** Whether the entry installs directly; authoritative over `download_url`. */
  installable?: boolean;
  category_label?: string | null;
}

export interface CatalogDetail extends CatalogEntry {
  overview: string;
  install_identifier: string | null;
}

export interface CatalogPageQuery {
  query?: string;
  /** Upstream sources to keep; empty or absent keeps every source. */
  sources?: string[];
  category?: string;
  /** 1-based page. */
  page: number;
  pageSize: number;
  forceRefresh?: boolean;
}

export interface CatalogPage {
  entries: CatalogEntry[];
  total: number;
  page: number;
  pageSize: number;
  totalPages: number;
  freshness: RegistryFreshness;
  /** Unix seconds of the oldest catalog fetch that answered. */
  fetchedAt: number | null;
  refreshing: boolean;
  lastError: RegistryErrorSummary | null;
}

interface RawCatalogPage {
  entries: CatalogEntry[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
  freshness: RegistryFreshness;
  fetched_at?: number | null;
  refreshing?: boolean;
  last_error?: RegistryErrorSummary | null;
}

/** A registry error as the core reports it: `SKILL_REGISTRY_<KIND>: message`. */
export interface ParsedRegistryError {
  kind: RegistryErrorKind | null;
  message: string;
  retryAfterSecs: number | null;
}

const REGISTRY_ERROR_PATTERN = /SKILL_REGISTRY_([A-Z_]+):\s*([\s\S]*)$/;
const RETRY_AFTER_PATTERN = /retry after (\d+)s/;

/** Read the typed kind, message and retry delay out of a failed registry call. */
export function parseRegistryError(error: unknown): ParsedRegistryError {
  const raw = error instanceof Error ? error.message : String(error ?? '');
  const match = REGISTRY_ERROR_PATTERN.exec(raw);
  const message = match ? match[2].trim() : raw;
  const retry = RETRY_AFTER_PATTERN.exec(message);
  return {
    kind: match ? (match[1].toLowerCase() as RegistryErrorKind) : null,
    message,
    retryAfterSecs: retry ? Number(retry[1]) : null,
  };
}

/** Whether an entry can be installed directly from the registry. */
export function isInstallable(entry: CatalogEntry): boolean {
  return entry.installable ?? Boolean(entry.download_url);
}

interface RegistryInstallResult {
  url: string;
  stdout: string;
  stderr: string;
  newSkills: string[];
}

interface RawRegistryInstallResult {
  url: string;
  stdout: string;
  stderr: string;
  new_skills: string[];
}

interface RegistryUninstallResult {
  name: string;
  removedPath: string;
  scope: string;
}

interface RawRegistryUninstallResult {
  name: string;
  removed_path: string;
  scope: string;
}

interface ControllerSchemaSummary {
  namespace: string;
  function: string;
  description: string;
  inputs: Array<Record<string, unknown>>;
  outputs: Array<Record<string, unknown>>;
}

interface Envelope<T> {
  data?: T;
}

function unwrap<T>(response: Envelope<T> | T): T {
  if (response && typeof response === 'object' && 'data' in response) {
    const env = response as Envelope<T>;
    if (env.data !== undefined) return env.data as T;
  }
  return response as T;
}

export const skillRegistryApi = {
  browsePage: async (query: CatalogPageQuery): Promise<CatalogPage> => {
    const text = query.query?.trim() ?? '';
    const sources = query.sources?.filter(Boolean) ?? [];
    log(
      'browsePage: query=%s sources=%d page=%d size=%d force=%s',
      text,
      sources.length,
      query.page,
      query.pageSize,
      Boolean(query.forceRefresh)
    );
    const params: Record<string, unknown> = { page: query.page, page_size: query.pageSize };
    if (text) params.query = text;
    if (sources.length > 0) params.sources = sources;
    if (query.category) params.category = query.category;
    if (query.forceRefresh) params.force_refresh = true;
    const response = await callCoreRpc<Envelope<RawCatalogPage> | RawCatalogPage>({
      method: text ? 'openhuman.skill_registry_search' : 'openhuman.skill_registry_browse',
      params,
      timeoutMs: CATALOG_RPC_TIMEOUT_MS,
    });
    const raw = unwrap(response);
    const page: CatalogPage = {
      entries: raw.entries ?? [],
      total: raw.total ?? 0,
      page: raw.page ?? query.page,
      pageSize: raw.page_size ?? query.pageSize,
      totalPages: raw.total_pages ?? 0,
      freshness: raw.freshness ?? 'live',
      fetchedAt: raw.fetched_at ?? null,
      refreshing: raw.refreshing ?? false,
      lastError: raw.last_error ?? null,
    };
    log(
      'browsePage: total=%d returned=%d freshness=%s error=%s',
      page.total,
      page.entries.length,
      page.freshness,
      page.lastError?.kind ?? 'none'
    );
    return page;
  },

  detail: async (entryId: string): Promise<CatalogDetail> => {
    log('detail: entryId=%s', entryId);
    const response = await callCoreRpc<Envelope<CatalogDetail> | CatalogDetail>({
      method: 'openhuman.skill_registry_detail',
      params: { entry_id: entryId },
      timeoutMs: CATALOG_RPC_TIMEOUT_MS,
    });
    return unwrap(response);
  },

  sources: async (): Promise<string[]> => {
    log('sources: request');
    const response = await callCoreRpc<Envelope<{ sources: string[] }> | { sources: string[] }>({
      method: 'openhuman.skill_registry_sources',
      timeoutMs: CATALOG_RPC_TIMEOUT_MS,
    });
    const result = unwrap(response);
    log('sources: count=%d', result.sources.length);
    return result.sources;
  },

  categories: async (): Promise<string[]> => {
    log('categories: request');
    const response = await callCoreRpc<
      Envelope<{ categories: string[] }> | { categories: string[] }
    >({ method: 'openhuman.skill_registry_categories', timeoutMs: CATALOG_RPC_TIMEOUT_MS });
    const result = unwrap(response);
    log('categories: count=%d', result.categories.length);
    return result.categories;
  },

  install: async (entryId: string): Promise<RegistryInstallResult> => {
    log('install: entryId=%s', entryId);
    const response = await callCoreRpc<
      Envelope<RawRegistryInstallResult> | RawRegistryInstallResult
    >({
      method: 'openhuman.skill_registry_install',
      params: { entry_id: entryId },
      timeoutMs: INSTALL_RPC_TIMEOUT_MS,
    });
    const raw = unwrap(response);
    const result: RegistryInstallResult = {
      url: raw.url,
      stdout: raw.stdout,
      stderr: raw.stderr,
      newSkills: raw.new_skills ?? [],
    };
    log('install: newSkills=%d', result.newSkills.length);
    return result;
  },

  uninstall: async (name: string): Promise<RegistryUninstallResult> => {
    log('uninstall: name=%s', name);
    const response = await callCoreRpc<
      Envelope<RawRegistryUninstallResult> | RawRegistryUninstallResult
    >({ method: 'openhuman.skill_registry_uninstall', params: { name } });
    const raw = unwrap(response);
    const result: RegistryUninstallResult = {
      name: raw.name,
      removedPath: raw.removed_path,
      scope: raw.scope,
    };
    log('uninstall: removedPath=%s', result.removedPath);
    return result;
  },

  schemas: async (): Promise<ControllerSchemaSummary[]> => {
    log('schemas: request');
    const response = await callCoreRpc<
      Envelope<{ schemas: ControllerSchemaSummary[] }> | { schemas: ControllerSchemaSummary[] }
    >({ method: 'openhuman.skill_registry_schemas' });
    const result = unwrap(response);
    log('schemas: count=%d', result.schemas.length);
    return result.schemas;
  },
};
