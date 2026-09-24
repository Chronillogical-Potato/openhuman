import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

/**
 * The reasoning panel's live label (and the running tool group) apply the
 * `shimmer` class from `tw-shimmer`, the dependency the assistant-ui registry
 * elements declare. This app once shipped those elements without it, so the
 * class matched nothing and a streaming trace looked identical to a settled
 * one. This pins both halves: the package is installed and the stylesheet
 * imports it.
 */

// Read from disk (tests run from `app/`): vitest stubs CSS imports, even
// `?raw`, to empty modules.
const read = (path: string) => readFileSync(resolve(path), 'utf8');

describe('tw-shimmer', () => {
  it('is imported by the app stylesheet', () => {
    expect(read('src/index.css')).toMatch(/@import\s+['"]tw-shimmer['"];/);
  });

  it('is a declared dependency that provides the `shimmer` utility', () => {
    const pkg = JSON.parse(read('package.json')) as {
      dependencies?: Record<string, string>;
    };
    expect(pkg.dependencies?.['tw-shimmer']).toBeTruthy();
    expect(read('node_modules/tw-shimmer/src/index.css')).toMatch(/@utility shimmer \{/);
  });
});
