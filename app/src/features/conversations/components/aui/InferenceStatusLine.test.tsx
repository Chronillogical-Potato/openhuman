/**
 * `InferenceStatusLine` renders every phase, including `thinking`.
 *
 * On `/chat` (`AssistantUiChat`) assistant-ui paints its own pulsing `●` during
 * the pre-first-token gap, so a `Thinking... (N)` caption there would be a
 * second indicator stacked under the library's. That duplicate is suppressed at
 * the assistant-ui caller (`AssistantUiInferenceStatus` returns null for
 * `thinking`), not by deleting the branch from this component, which stays a
 * complete renderer for any caller that has no indicator of its own.
 */
import { screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { I18nProvider } from '../../../../lib/i18n/I18nContext';
import { renderWithProviders } from '../../../../test/test-utils';
import { InferenceStatusLine } from './InferenceStatusLine';

// `I18nProvider` reads `state.locale.current`, so the line needs a store; the
// shared helper supplies the same reducer set the app mounts.
function renderLine(status: { phase: string; iteration: number }) {
  return renderWithProviders(
    <I18nProvider>
      <InferenceStatusLine status={status} />
    </I18nProvider>
  );
}

describe('InferenceStatusLine — the legacy surface still gets a thinking caption', () => {
  it('captions the thinking phase with the iteration count', () => {
    renderLine({ phase: 'thinking', iteration: 3 });
    // The legacy surface has no library dot, so a bare pulse with no words is
    // indistinguishable from a stalled turn.
    expect(screen.getByTestId('inference-status-line').textContent).toContain('3');
    expect(screen.getByTestId('inference-status-line').textContent).toMatch(/Thinking/i);
  });

  it('captions the thinking phase without a count before the first iteration', () => {
    renderLine({ phase: 'thinking', iteration: 0 });
    const text = screen.getByTestId('inference-status-line').textContent ?? '';
    expect(text).toMatch(/Thinking/i);
    // `Thinking... (0)` would be worse than no number at all.
    expect(text).not.toContain('(0)');
  });

  it('renders the line with a caption, not an empty pulse', () => {
    // The regression this guards is specifically "renders, but says nothing":
    // the element was still present, so a presence assertion would have passed
    // while the user saw a bare dot.
    renderLine({ phase: 'thinking', iteration: 1 });
    const text = (screen.getByTestId('inference-status-line').textContent ?? '').trim();
    expect(text.length).toBeGreaterThan(0);
  });
});
