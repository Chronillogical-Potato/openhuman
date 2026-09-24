/**
 * The "never again" guard for tool labels.
 *
 * `__fixtures__/coreToolNames.json` is every tool name the core registers. It
 * is written and checked by the Rust test next to the core's tool registry
 * (`UPDATE_TOOL_CATALOG=1` regenerates it), so a tool added to the core
 * without updating the fixture fails there, and a fixture name the registry
 * here cannot describe fails here. Between them a new core tool cannot reach
 * the chat as a raw identifier.
 */
import { describe, expect, it } from 'vitest';

import coreToolNames from './__fixtures__/coreToolNames.json';
import { describeToolCall, type ToolCallStatus, toolLabel } from './toolPresentation';

const NAMES = [...(coreToolNames as string[])].sort();
const STATUSES: ToolCallStatus[] = ['running', 'success', 'error'];
/** Brand and protocol words allowed to stay upper-case inside a label. */
const ALLOWED_CAPS = new Set(['MCP', 'CSV', 'API']);

describe('core tool catalog', () => {
  it('is not empty', () => {
    expect(NAMES.length).toBeGreaterThan(100);
  });

  it.each(NAMES)('%s is described by the registry, not the generic fallback', name => {
    const presentation = describeToolCall({ name });
    expect(presentation.source).not.toBe('fallback');
    expect(presentation.source).not.toBe('server');
    expect(presentation.icon).toBeTruthy();
  });

  it.each(NAMES)('%s reads as a human label in every state', name => {
    const labels = STATUSES.map(status => toolLabel(describeToolCall({ name, status })));
    for (const label of labels) {
      expect(label.trim().length).toBeGreaterThan(0);
      expect(label).not.toBe(name);
      expect(label).not.toContain('_');
      expect(label).not.toMatch(/^Using [A-Z]\w*ing\b/);
      const shouting = label
        .split(/\s+/)
        .filter(word => /^[A-Z]{2,}$/.test(word) && !ALLOWED_CAPS.has(word));
      expect(shouting).toEqual([]);
    }
    // The tense changes as the call settles.
    expect(labels[0]).not.toBe(labels[1]);
  });
});
