import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

// Read from disk (tests run from `app/`): vitest stubs CSS imports, even
// `?raw`, to empty modules.
const css = readFileSync(resolve('src/index.css'), 'utf8');

/**
 * The reasoning panel's live label (and the running tool group) apply the
 * `shimmer` class. The assistant-ui registry components expect it from the
 * `tw-shimmer` plugin, which this app never installed, so the class used to
 * match nothing and a streaming trace looked identical to a settled one. This
 * pins the in-house `@utility shimmer` so it cannot silently disappear again.
 */
describe('shimmer utility', () => {
  const block = (() => {
    const start = css.indexOf('@utility shimmer {');
    if (start === -1) return '';
    // The utility body runs to the first closing brace at column 0.
    const end = css.indexOf('\n}', start);
    return css.slice(start, end + 2);
  })();

  it('is defined in index.css', () => {
    expect(block, '@utility shimmer is missing from index.css').not.toBe('');
  });

  it('clips an animated gradient to the text', () => {
    expect(block).toMatch(/background-clip:\s*text/);
    expect(block).toMatch(/color:\s*transparent/);
    expect(block).toMatch(/animation:\s*var\(--animate-shimmer\)/);
  });

  it('animates with keyframes and a theme variable that exist', () => {
    expect(css).toMatch(/--animate-shimmer:\s*shimmer\s/);
    expect(css).toMatch(/@keyframes shimmer\s*\{/);
  });

  it('stands still for reduced-motion users', () => {
    expect(block).toMatch(/prefers-reduced-motion:\s*reduce[\s\S]*animation:\s*none/);
  });
});
