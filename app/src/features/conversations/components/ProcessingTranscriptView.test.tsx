import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { ToolTimelineEntry } from '../../../store/chatRuntimeSlice';
import { ProcessingTranscriptView } from './ProcessingTranscriptView';

// Echo i18n: return the fallback when one is provided (so localized copy keys
// resolve to the English source we pass), otherwise the key itself. This lets
// us assert on the human strings carried on the failure payload.
vi.mock('../../../lib/i18n/I18nContext', () => ({
  useT: () => ({ t: (key: string, fallback?: string) => fallback ?? key, locale: 'en' }),
}));

function failedEntry(overrides: Partial<ToolTimelineEntry> = {}): ToolTimelineEntry {
  return {
    id: 'call-1',
    name: 'read_file',
    round: 1,
    seq: 0,
    status: 'error',
    failure: {
      class: 'MissingPermission',
      category: 'NeedsUserConfirmation',
      recoverable: false,
      causePlain: 'No permission yet.',
      nextAction: 'Grant it, then retry.',
    },
    ...overrides,
  };
}

describe('ProcessingTranscriptView tool failure explanation', () => {
  it('renders the cause + next-action under a failed tool row', () => {
    // Empty transcript → single tool group over all entries (legacy path).
    render(<ProcessingTranscriptView transcript={[]} entries={[failedEntry()]} />);

    const failure = screen.getByTestId('processing-tool-failure');
    expect(failure).toBeTruthy();
    expect(failure.textContent).toContain('No permission yet.');
    expect(failure.textContent).toContain('Grant it, then retry.');
  });

  it('falls back to the plain English copy for an unrecognized class', () => {
    render(
      <ProcessingTranscriptView
        transcript={[]}
        entries={[
          failedEntry({
            failure: {
              class: 'SomethingBrandNew',
              category: 'Recoverable',
              recoverable: true,
              causePlain: 'Mystery cause.',
              nextAction: 'Mystery next.',
            },
          }),
        ]}
      />
    );

    const failure = screen.getByTestId('processing-tool-failure');
    expect(failure.textContent).toContain('Mystery cause.');
    expect(failure.textContent).toContain('Mystery next.');
  });

  it('does not render the failure block for a successful entry', () => {
    render(
      <ProcessingTranscriptView
        transcript={[]}
        entries={[failedEntry({ status: 'success', failure: undefined })]}
      />
    );
    expect(screen.queryByTestId('processing-tool-failure')).toBeNull();
  });
});

describe('ProcessingTranscriptView live thinking', () => {
  const thought = (seq: number, text: string) =>
    ({ kind: 'thinking', round: 1, seq, text }) as const;

  it('renders the trailing thought expanded while the turn is live', () => {
    render(
      <ProcessingTranscriptView
        transcript={[thought(0, 'The user wants a week in Kashmir in October.')]}
        entries={[]}
        live
      />
    );
    const live = screen.getByTestId('processing-thinking-live');
    // Rendered as a step of the live reasoning panel (its title drops the
    // sentence's closing period).
    expect(live.textContent).toContain('The user wants a week in Kashmir in October');
    expect(live.getAttribute('aria-busy')).toBe('true');
    expect(live.querySelector('[data-shimmer]')).not.toBeNull();
    // No settled row for the same thought.
    expect(screen.queryByTestId('processing-thinking')).toBeNull();
  });

  it('renders settled thoughts as a quiet, non-collapsible reasoning panel', () => {
    render(
      <ProcessingTranscriptView
        transcript={[thought(0, 'Settled reasoning that should stay quiet.')]}
        entries={[]}
      />
    );
    expect(screen.queryByTestId('processing-thinking-live')).toBeNull();
    const settled = screen.getByTestId('processing-thinking');
    // The rail keeps the trail visible: no disclosure, no live shimmer.
    expect(settled.getAttribute('data-variant')).toBe('static');
    expect(settled.querySelector('button')).toBeNull();
    expect(settled.querySelector('[data-shimmer]')).toBeNull();
    expect(settled.textContent).toContain('Settled reasoning that should stay quiet');
  });

  it('labels a settled thought with its recorded duration', () => {
    render(
      <ProcessingTranscriptView
        transcript={[{ ...thought(0, '**Planning**\nok'), startedAt: 1_000, endedAt: 13_000 }]}
        entries={[]}
      />
    );
    const settled = screen.getByTestId('processing-thinking');
    expect(settled.querySelector('[data-swap-layer="resting"]')?.textContent).toBe(
      'chat.reasoning.thoughtFor'.replace('{n}', '12s')
    );
    expect(settled.querySelector('[data-slot="reasoning-step-title"]')?.textContent).toBe(
      'Planning'
    );
  });

  it('only expands the LAST thought while live; earlier ones stay collapsed', () => {
    render(
      <ProcessingTranscriptView
        transcript={[
          thought(0, 'First pass of reasoning.'),
          { kind: 'narration', round: 1, seq: 1, text: 'Let me look that up.' },
          thought(2, 'Second pass of reasoning.'),
        ]}
        entries={[]}
        live
      />
    );
    expect(screen.getAllByTestId('processing-thinking')).toHaveLength(1);
    expect(screen.getByTestId('processing-thinking').textContent).toContain('First pass');
    expect(screen.getByTestId('processing-thinking-live').textContent).toContain('Second pass');
  });

  it('keeps a long live thought whole inside a bounded, bottom-pinned scroll region', () => {
    const long = 'x'.repeat(2000) + 'TAIL';
    render(<ProcessingTranscriptView transcript={[thought(0, long)]} entries={[]} live />);
    const live = screen.getByTestId('processing-thinking-live');
    expect(live.textContent).toContain('TAIL');
    const scroll = live.querySelector('[data-slot="reasoning-panel-scroll"]');
    expect(scroll?.className).toContain('max-h-80');
    expect(scroll?.className).toContain('overflow-y-auto');
  });
});
