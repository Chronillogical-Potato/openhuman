import { describe, expect, it } from 'vitest';

import { MEMORY_CHIPS, resolveMemoryChip } from './memoryChips';

describe('memoryChips', () => {
  it('keeps exactly Provider, Conversations and Files', () => {
    expect(MEMORY_CHIPS).toEqual(['engine', 'conversations', 'brain']);
  });

  it('resolves current chips to themselves', () => {
    for (const chip of MEMORY_CHIPS) expect(resolveMemoryChip(chip)).toBe(chip);
  });

  it.each(['ask', 'explorer', 'learnings', 'background', 'settings', 'graph', 'goals', 'context'])(
    'sends retired chip %s to Conversations',
    legacy => {
      expect(resolveMemoryChip(legacy)).toBe('conversations');
    }
  );

  it.each(['migration', 'documents', 'sources', 'sync', 'history'])(
    'sends retired chip %s to Files',
    legacy => {
      expect(resolveMemoryChip(legacy)).toBe('brain');
    }
  );

  it('returns null for empty and unknown values', () => {
    expect(resolveMemoryChip(null)).toBeNull();
    expect(resolveMemoryChip(undefined)).toBeNull();
    expect(resolveMemoryChip('')).toBeNull();
    expect(resolveMemoryChip('bogus')).toBeNull();
  });
});
