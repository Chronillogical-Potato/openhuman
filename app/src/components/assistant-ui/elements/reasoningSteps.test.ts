import { describe, expect, it } from 'vitest';

import en from '../../../lib/i18n/en';
import {
  DERIVED_TITLE_MAX,
  formatElapsed,
  latestHeading,
  parseReasoningParts,
  parseReasoningSteps,
  reasoningSpan,
  thoughtForLabel,
} from './reasoningSteps';

const t = (key: string) => en[key] ?? key;

describe('parseReasoningSteps', () => {
  it('returns no steps for empty or whitespace-only text', () => {
    expect(parseReasoningSteps('')).toEqual([]);
    expect(parseReasoningSteps('  \n\n ')).toEqual([]);
  });

  it('splits Codex-style bold heading lines into titled steps', () => {
    const steps = parseReasoningSteps(
      '**Reading the request**\nThe user wants a summary.\n\n**Planning the fix**\nPatch the parser first.'
    );
    expect(steps).toEqual([
      { title: 'Reading the request', body: 'The user wants a summary.' },
      { title: 'Planning the fix', body: 'Patch the parser first.' },
    ]);
  });

  it('accepts markdown headings and a trailing colon on bold headings', () => {
    const steps = parseReasoningSteps('## Scope\nOnly the UI.\n**Risks:**\nNone.');
    expect(steps.map(s => s.title)).toEqual(['Scope', 'Risks']);
    expect(steps.every(s => !s.derived)).toBe(true);
  });

  it('keeps an inline bold phrase as body text rather than a heading', () => {
    const steps = parseReasoningSteps('**Plan**\nUse **bold** words inline.');
    expect(steps).toHaveLength(1);
    expect(steps[0]?.body).toBe('Use **bold** words inline.');
  });

  it('titles untitled paragraphs by their first sentence and keeps the rest as body', () => {
    const steps = parseReasoningSteps(
      'Check the cache first. It may be stale.\n\nThen rebuild the index.'
    );
    expect(steps).toEqual([
      { title: 'Check the cache first', body: 'It may be stale.', derived: true },
      { title: 'Then rebuild the index', body: '', derived: true },
    ]);
  });

  it('shortens a long first sentence on a word boundary and keeps the whole paragraph', () => {
    const long =
      'This is a deliberately long opening sentence that keeps going well past the title limit for sure';
    const [step] = parseReasoningSteps(long);
    expect(step?.derived).toBe(true);
    expect(step?.title.endsWith('…')).toBe(true);
    expect(step!.title.length).toBeLessThanOrEqual(DERIVED_TITLE_MAX + 1);
    expect(step?.body).toBe(long);
  });

  it('turns preamble before the first heading into its own step', () => {
    const steps = parseReasoningSteps('Quick look first.\n**Deep dive**\nDetails.');
    expect(steps.map(s => s.title)).toEqual(['Quick look first', 'Deep dive']);
  });

  it('leaves a heading that is still streaming in (unclosed) as body text', () => {
    const steps = parseReasoningSteps('**Planning the');
    expect(steps).toHaveLength(1);
    expect(steps[0]?.derived).toBe(true);
    expect(latestHeading(steps)).toBeUndefined();
  });

  it('never drops text: every non-heading character lands in a title or a body', () => {
    const text = '**A**\none\n\ntwo\n**B**\nthree';
    const joined = parseReasoningSteps(text)
      .map(s => `${s.title} ${s.body}`)
      .join(' ');
    for (const word of ['A', 'one', 'two', 'B', 'three']) expect(joined).toContain(word);
  });
});

describe('parseReasoningParts / latestHeading', () => {
  it('concatenates the steps of several reasoning parts in order', () => {
    const steps = parseReasoningParts(['**First**\na', '**Second**\nb']);
    expect(steps.map(s => s.title)).toEqual(['First', 'Second']);
  });

  it('picks the newest real heading, skipping derived titles', () => {
    const steps = parseReasoningParts(['**Planning**\nx', 'an untitled follow-up sentence.']);
    expect(latestHeading(steps)).toBe('Planning');
  });
});

describe('reasoningSpan', () => {
  it('spans earliest start to latest end across parts', () => {
    expect(
      reasoningSpan([
        { startedAt: 2_000, endedAt: 5_000 },
        undefined,
        { startedAt: 1_000, endedAt: 9_000 },
      ])
    ).toEqual({ startedAt: 1_000, endedAt: 9_000 });
  });

  it('is undefined when no part carries a start (legacy rows)', () => {
    expect(reasoningSpan([undefined, { endedAt: 3 }])).toBeUndefined();
    expect(reasoningSpan([])).toBeUndefined();
  });
});

describe('duration labels', () => {
  it('formats seconds and minutes compactly', () => {
    expect(formatElapsed(0, t)).toBe('0s');
    expect(formatElapsed(12_400, t)).toBe('12s');
    expect(formatElapsed(72_000, t)).toBe('1m 12s');
  });

  it('says "Thought for Ns" once the duration is known', () => {
    expect(thoughtForLabel(12_000, t)).toBe('Thought for 12s');
    expect(thoughtForLabel(125_000, t)).toBe('Thought for 2m 5s');
  });

  it('says "Thought briefly" under a second and "Thought" when unknown', () => {
    expect(thoughtForLabel(400, t)).toBe('Thought briefly');
    expect(thoughtForLabel(undefined, t)).toBe('Thought');
    expect(thoughtForLabel(-5, t)).toBe('Thought');
    expect(thoughtForLabel(Number.NaN, t)).toBe('Thought');
  });
});
