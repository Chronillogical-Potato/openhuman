import { describe, expect, it } from 'vitest';

import { formatBytes } from './format';

describe('formatBytes', () => {
  it('returns 0 B for missing, negative, or non-finite input', () => {
    expect(formatBytes(undefined)).toBe('0 B');
    expect(formatBytes(null)).toBe('0 B');
    expect(formatBytes(-1)).toBe('0 B');
    expect(formatBytes(Number.NaN)).toBe('0 B');
    expect(formatBytes(Number.POSITIVE_INFINITY)).toBe('0 B');
  });

  it('keeps sub-kilobyte values in bytes', () => {
    expect(formatBytes(512)).toBe('512 B');
  });

  it('uses one decimal below 10 and none above', () => {
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(20 * 1024 * 1024)).toBe('20 MB');
    expect(formatBytes(3 * 1024 ** 4)).toBe('3.0 TB');
  });
});
