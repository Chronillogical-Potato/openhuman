/**
 * The Memory page's chip ids (`?brain=<chip>`) and the mapping that keeps old
 * deep links working. Shared by the page and the `/brain` redirect.
 */

export type MemoryChip = 'engine' | 'conversations' | 'brain';

export const MEMORY_CHIPS: readonly MemoryChip[] = ['engine', 'conversations', 'brain'];

/**
 * Retired chips → their current home. The page keeps three tabs: Provider
 * (`engine`), Conversations and Files (`brain`). Importing and migrating old
 * memory (`migration`) and every way of managing documents (`documents`,
 * `sources`, `sync`, `history`) live on Files; every other retired chip
 * (ask, explorer, learnings, background, settings, v1's graph, goals and
 * context) lands on Conversations, the closest surviving view of what memory
 * holds and how it is filled.
 */
const LEGACY_CHIPS: Record<string, MemoryChip> = {
  migration: 'brain',
  documents: 'brain',
  sources: 'brain',
  sync: 'brain',
  history: 'brain',
  ask: 'conversations',
  explorer: 'conversations',
  learnings: 'conversations',
  background: 'conversations',
  settings: 'conversations',
  graph: 'conversations',
  goals: 'conversations',
  context: 'conversations',
};

/** Resolve a raw `?brain=` value to a chip, or `null` when it names none. */
export function resolveMemoryChip(raw: string | null | undefined): MemoryChip | null {
  if (!raw) return null;
  if ((MEMORY_CHIPS as readonly string[]).includes(raw)) return raw as MemoryChip;
  return LEGACY_CHIPS[raw] ?? null;
}
