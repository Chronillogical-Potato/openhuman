import { describe, expect, it } from 'vitest';

import { emptySessionTokenUsage } from '../../../../store/chatRuntimeSlice';
import { cacheHitLabel, contextUsageFromTokenUsage } from './ContextWindowPill';

describe('contextUsageFromTokenUsage', () => {
  it('keeps cached input separate from fresh input', () => {
    expect(
      contextUsageFromTokenUsage({
        ...emptySessionTokenUsage(),
        inputTokens: 1_000,
        cachedTokens: 400,
        outputTokens: 250,
        costUsd: 0.02,
        contextWindow: 128_000,
        lastTurnContextUsed: 900,
      })
    ).toEqual({
      used: 900,
      limit: 128_000,
      input: 600,
      cachedInput: 400,
      output: 250,
      costUsd: 0.02,
    });
  });

  it('preserves an unknown context limit as zero', () => {
    expect(contextUsageFromTokenUsage(emptySessionTokenUsage()).limit).toBe(0);
  });

  it('uses the selected model context window instead of stale turn usage', () => {
    const usage = { ...emptySessionTokenUsage(), contextWindow: 200_000 };
    expect(contextUsageFromTokenUsage(usage, 128_000).limit).toBe(128_000);
    expect(contextUsageFromTokenUsage(usage, null).limit).toBe(0);
  });
});

describe('cacheHitLabel', () => {
  // These assert the STRING the "Cached input" row renders, not the
  // `cachedInput` field. A field assertion passes while the row is still an
  // absolute count, which is the bug (#6461) — so it would be vacuous.
  it('renders cache reads as a share of the turn total input', () => {
    // 400 cached out of 1000 reported input. Note the denominator is
    // `input + cachedInput`, NOT `input` (600) — against `input` this would
    // read 67%.
    expect(cacheHitLabel({ input: 600, cachedInput: 400 })).toBe('40%');
  });

  it('renders an em dash, not 0%, when no input has been reported', () => {
    expect(cacheHitLabel({ input: 0, cachedInput: 0 })).toBe('—');
    expect(cacheHitLabel(contextUsageFromTokenUsage(emptySessionTokenUsage()))).toBe('—');
  });

  it('renders 0% for a reported turn that got no cache hit', () => {
    // Distinct from the em dash above: this turn measured a real 0% hit rate.
    expect(cacheHitLabel({ input: 1_000, cachedInput: 0 })).toBe('0%');
  });

  it('rounds a negligible cache hit down to 0% rather than up', () => {
    expect(cacheHitLabel({ input: 999, cachedInput: 1 })).toBe('0%');
  });

  it('never exceeds 100% when a provider over-reports cached input', () => {
    // `cachedTokens` (500) > `inputTokens` (100). `contextUsageFromTokenUsage`
    // floors `input` at 0, so the share lands on 100% instead of 500%. This is
    // the reachable form of the over-report; the `Math.min` in `cacheHitLabel`
    // is unreachable through this path because the denominator contains the
    // numerator.
    const usage = contextUsageFromTokenUsage({
      ...emptySessionTokenUsage(),
      inputTokens: 100,
      cachedTokens: 500,
    });
    expect(usage.input).toBe(0);
    expect(cacheHitLabel(usage)).toBe('100%');
  });
});
