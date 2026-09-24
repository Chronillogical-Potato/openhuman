import type { ToolCallMessagePartProps } from '@assistant-ui/react';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import {
  MemoryHybridSearchCall,
  MemoryRecallCall,
  MemoryStoreCall,
  memoryToolChips,
} from './ChatMemoryChips';

/** The prop fields every `ToolCallMessagePartComponent` requires, beyond `args`/`result`. */
function toolCallProps(
  toolName: string,
  args: unknown,
  result: unknown
): ToolCallMessagePartProps {
  return {
    type: 'tool-call',
    toolName,
    toolCallId: `${toolName}-1`,
    args: args as never,
    argsText: '{}',
    result,
    status: { type: 'complete' },
    addResult: () => {},
    resume: () => {},
    respondToApproval: () => Promise.resolve(),
  };
}

describe('memoryToolChips', () => {
  it('builds one "added" chip for a memory_store call, keyed by its key', () => {
    const chips = memoryToolChips('memory_store', { key: 'favorite_color', content: 'blue' }, undefined);
    expect(chips).toEqual([{ id: 'store:favorite_color', text: 'favorite_color', change: 'added' }]);
  });

  it('builds one "existing" chip per hit for memory_recall / memory_hybrid_search', () => {
    const chips = memoryToolChips('memory_recall', undefined, [
      { key: 'favorite_color', text: 'blue' },
      { key: 'timezone', text: 'UTC+2' },
    ]);
    expect(chips.map(c => c.text)).toEqual(['favorite_color', 'timezone']);
    expect(chips.every(c => c.change === 'existing')).toBe(true);
  });

  it('returns nothing for a tool name it does not know', () => {
    expect(memoryToolChips('memory_forget', {}, undefined)).toEqual([]);
  });
});

describe('memory tool call renders', () => {
  it('MemoryStoreCall renders the vendored memory-chips element', () => {
    render(<MemoryStoreCall {...toolCallProps('memory_store', { key: 'favorite_color' }, undefined)} />);
    expect(screen.getByText('favorite_color')).toBeTruthy();
  });

  it('MemoryRecallCall renders nothing for an empty result', () => {
    const { container } = render(
      <MemoryRecallCall {...toolCallProps('memory_recall', undefined, [])} />
    );
    expect(container.querySelector('[data-slot="memory-chips"]')).toBeNull();
  });

  it('MemoryHybridSearchCall renders one chip per hit', () => {
    render(
      <MemoryHybridSearchCall
        {...toolCallProps('memory_hybrid_search', undefined, { results: [{ key: 'k1' }, { key: 'k2' }] })}
      />
    );
    expect(screen.getByText('k1')).toBeTruthy();
    expect(screen.getByText('k2')).toBeTruthy();
  });
});
