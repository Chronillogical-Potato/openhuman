/**
 * Labels for the explorer's facets and their values. Literal `t()` keys (not
 * built from the facet) so the i18n scanner sees every one of them.
 */
import type { Facet, ItemKind, PathStep, SourceKind } from '../../services/api/memoryApi';
import { kindLabel } from './memoryFormat';
import { sourceKindLabel } from './memorySourceLabels';

type Translate = (key: string, fallback?: string) => string;

export function facetLabel(facet: Facet, t: Translate): string {
  switch (facet) {
    case 'kind':
      return t('memoryPage.explorer.facet.kind');
    case 'source':
      return t('memoryPage.explorer.facet.source');
    case 'source_id':
      return t('memoryPage.explorer.facet.sourceId');
    case 'workspace':
      return t('memoryPage.explorer.facet.workspace');
    case 'folder':
      return t('memoryPage.explorer.facet.folder');
    case 'file_path':
      return t('memoryPage.explorer.facet.file');
    case 'language':
      return t('memoryPage.explorer.facet.language');
    case 'repo':
      return t('memoryPage.explorer.facet.repo');
    case 'url':
      return t('memoryPage.explorer.facet.url');
    case 'thread':
      return t('memoryPage.explorer.facet.thread');
    case 'agent':
      return t('memoryPage.explorer.facet.agent');
    case 'tool_call':
      return t('memoryPage.explorer.facet.toolCall');
    case 'tag':
      return t('memoryPage.explorer.facet.tag');
    case 'namespace':
      return t('memoryPage.explorer.facet.namespace');
    default:
      return String(facet);
  }
}

function sourceLabel(kind: SourceKind, t: Translate): string {
  switch (kind) {
    case 'conversation':
      return t('memoryPage.explorer.source.conversation');
    case 'agent':
      return t('memoryPage.explorer.source.agent');
    case 'import':
      return t('memoryPage.explorer.source.import');
    default:
      return sourceKindLabel(kind, t);
  }
}

/** A bucket value as a reader sees it: kinds and sources translated, the rest verbatim. */
export function facetValueLabel(facet: Facet, value: string, t: Translate): string {
  if (facet === 'kind') return kindLabel(value as ItemKind, t);
  if (facet === 'source') return sourceLabel(value as SourceKind, t);
  if (facet === 'namespace') return namespaceLabel(value, t);
  return value;
}

/** A memory node as a reader sees it: the shared root named, agents verbatim. */
export function namespaceLabel(value: string, t: Translate): string {
  return value === 'root' ? t('memoryPage.explorer.namespace.root') : value;
}

/** Whether values of `facet` are identifiers or paths, shown in a monospace face. */
export function isMonoFacet(facet: Facet): boolean {
  return facet !== 'kind' && facet !== 'source' && facet !== 'tag';
}

/** The order the explorer suggests drilling in. */
const DRILL_ORDER: readonly Facet[] = [
  'kind',
  'source',
  'source_id',
  'folder',
  'file_path',
  'thread',
  'tag',
];

/** The facet to group by next under `path`: the first suggested one not yet chosen. */
export function nextFacet(path: readonly PathStep[], all: readonly Facet[]): Facet {
  const used = new Set(path.map(step => step.facet));
  return DRILL_ORDER.find(f => !used.has(f)) ?? all.find(f => !used.has(f)) ?? 'kind';
}
