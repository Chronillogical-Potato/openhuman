import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ReasoningPanel } from './reasoning-panel';
import { ReasoningTrace, ReasoningTraceText } from './reasoning-trace';

/**
 * Regression coverage for the static reasoning panel (assistant-ui
 * `elements-reasoning-panel`) and the `ReasoningTrace` wrapper every surface
 * renders through. These pin the behaviours the reasoning boxes used to get
 * wrong: a hard-coded "Reasoning" label, no duration ever shown, a shimmer
 * class that styled nothing, and one opaque block instead of titled steps.
 */

const STEPS = [
  { title: 'Reading the request', body: 'The user wants **a summary**.' },
  { title: 'Planning the fix', body: '- patch parser\n- add tests' },
];

const trigger = () => screen.getByRole('button');
// `SwapLabel` stacks the live and resting labels as two layers and hides the
// inactive one with aria-hidden.
const liveLayer = (root: HTMLElement) =>
  root.querySelector('[data-slot="reasoning-panel-live-label"]')!.parentElement as HTMLElement;
const restingLayer = (root: HTMLElement) =>
  root.querySelector('[data-slot="reasoning-panel-resting-label"]')!.parentElement as HTMLElement;

describe('ReasoningPanel (collapsible)', () => {
  it('renders titled steps with markdown bodies when open', () => {
    render(
      <ReasoningPanel
        steps={STEPS}
        streaming={false}
        defaultOpen
        liveLabel="Thinking"
        restingLabel="Thought for 12s"
      />
    );
    const titles = screen.getAllByText(/Reading the request|Planning the fix/);
    expect(titles.map(el => el.textContent)).toEqual(['Reading the request', 'Planning the fix']);
    // Body markdown is rendered, not shown as raw syntax.
    expect(screen.getByText('a summary').tagName).toBe('STRONG');
    expect(screen.getAllByRole('listitem').some(li => li.textContent === 'patch parser')).toBe(
      true
    );
  });

  it('is collapsed by default once settled and shows the resting label', () => {
    const { container } = render(
      <ReasoningPanel
        steps={STEPS}
        streaming={false}
        liveLabel="Thinking"
        restingLabel="Thought for 12s"
        data-testid="panel"
      />
    );
    expect(trigger().getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByText('Reading the request')).toBeNull();
    const root = screen.getByTestId('panel');
    expect(restingLayer(root).textContent).toBe('Thought for 12s');
    expect(restingLayer(root).getAttribute('aria-hidden')).toBe('false');
    expect(liveLayer(root).getAttribute('aria-hidden')).toBe('true');
    expect(container.querySelector('.shimmer')).toBeNull();
  });

  it('toggles open and closed from the trigger', () => {
    render(
      <ReasoningPanel steps={STEPS} streaming={false} liveLabel="Thinking" restingLabel="Thought" />
    );
    fireEvent.click(trigger());
    expect(trigger().getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByText('Planning the fix')).toBeTruthy();
    fireEvent.click(trigger());
    expect(trigger().getAttribute('aria-expanded')).toBe('false');
  });

  it('opens while streaming with a shimmering live label and elapsed badge', () => {
    const { container } = render(
      <ReasoningPanel
        steps={STEPS}
        streaming
        liveLabel="Planning the fix"
        restingLabel="Thought"
        elapsed="4s"
        data-testid="panel"
      />
    );
    const root = screen.getByTestId('panel');
    expect(trigger().getAttribute('aria-expanded')).toBe('true');
    expect(liveLayer(root).getAttribute('aria-hidden')).toBe('false');
    const shimmer = container.querySelector('.shimmer');
    expect(shimmer?.textContent).toBe('Planning the fix');
    expect(shimmer?.className).toContain('shimmer');
    expect(root.querySelector('[data-slot="reasoning-panel-elapsed"]')?.textContent).toBe('4s');
    // The newest step is the active one on the timeline.
    const steps = root.querySelectorAll('[data-slot="reasoning-panel-step"]');
    expect(steps[steps.length - 1]?.hasAttribute('data-active')).toBe(true);
    expect(steps[0]?.hasAttribute('data-active')).toBe(false);
  });

  it('collapses when streaming ends, unless the reader toggled it', () => {
    const props = { steps: STEPS, liveLabel: 'Thinking', restingLabel: 'Thought for 3s' };
    const onAnimationStart = vi.fn();
    const { rerender } = render(
      <ReasoningPanel {...props} streaming onAnimationStart={onAnimationStart} />
    );
    expect(trigger().getAttribute('aria-expanded')).toBe('true');
    rerender(<ReasoningPanel {...props} streaming={false} onAnimationStart={onAnimationStart} />);
    expect(trigger().getAttribute('aria-expanded')).toBe('false');
    expect(onAnimationStart).toHaveBeenCalledTimes(1);

    // A second instance the reader keeps open across the transition.
    const second = render(<ReasoningPanel {...props} streaming data-testid="kept" />);
    const keptTrigger = second.getByTestId('kept').querySelector('button')!;
    fireEvent.click(keptTrigger); // close
    fireEvent.click(keptTrigger); // reopen: the reader has taken over
    second.rerender(<ReasoningPanel {...props} streaming={false} data-testid="kept" />);
    expect(keptTrigger.getAttribute('aria-expanded')).toBe('true');
  });

  it('disables the trigger and hides the chevron when there are no steps yet', () => {
    const { container } = render(
      <ReasoningPanel steps={[]} streaming liveLabel="Thinking" restingLabel="Thought" />
    );
    expect((trigger() as HTMLButtonElement).disabled).toBe(true);
    expect(container.querySelector('svg')).toBeNull();
  });

  it('reveals only visibleSteps, clamping out-of-range values', () => {
    const { rerender } = render(
      <ReasoningPanel
        steps={STEPS}
        visibleSteps={1}
        streaming
        liveLabel="Thinking"
        restingLabel="Thought"
      />
    );
    expect(screen.getAllByRole('listitem', { hidden: true }).length).toBeGreaterThanOrEqual(1);
    expect(screen.queryByText('Planning the fix')).toBeNull();
    rerender(
      <ReasoningPanel
        steps={STEPS}
        visibleSteps={-3}
        streaming
        liveLabel="Thinking"
        restingLabel="Thought"
      />
    );
    expect(screen.queryByText('Reading the request')).toBeNull();
  });
});

describe('ReasoningPanel (non-collapsible)', () => {
  it('renders the header and steps inline with no disclosure button', () => {
    render(
      <ReasoningPanel
        steps={STEPS}
        streaming={false}
        collapsible={false}
        liveLabel="Thinking"
        restingLabel="Thought for 12s"
        data-testid="static"
      />
    );
    expect(screen.queryByRole('button')).toBeNull();
    const root = screen.getByTestId('static');
    expect(root.getAttribute('data-variant')).toBe('static');
    expect(screen.getByText('Reading the request')).toBeTruthy();
    expect(screen.getByText('Planning the fix')).toBeTruthy();
    expect(restingLayer(root).textContent).toBe('Thought for 12s');
  });

  it('bounds and marks a live trace as busy while streaming', () => {
    render(
      <ReasoningPanel
        steps={STEPS}
        streaming
        collapsible={false}
        liveLabel="Planning the fix"
        restingLabel="Thought"
        data-testid="static"
      />
    );
    const root = screen.getByTestId('static');
    expect(root.getAttribute('aria-busy')).toBe('true');
    expect(root.querySelector('[data-slot="reasoning-panel-scroll"]')?.className).toContain(
      'max-h-80'
    );
  });
});

describe('ReasoningTrace', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-09-24T10:00:10Z'));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  const start = Date.parse('2026-09-24T10:00:00Z');

  it('shows "Thought for Ns" from the recorded timestamps once settled', () => {
    render(
      <ReasoningTrace
        texts={['**Reading**\nok']}
        timings={[{ startedAt: start, endedAt: start + 12_000 }]}
        streaming={false}
        data-testid="trace"
      />
    );
    expect(restingLayer(screen.getByTestId('trace')).textContent).toBe('Thought for 12s');
  });

  it('spans several parts: earliest start to latest end', () => {
    render(
      <ReasoningTrace
        texts={['**A**\na', '**B**\nb']}
        timings={[
          { startedAt: start, endedAt: start + 2_000 },
          { startedAt: start + 30_000, endedAt: start + 75_000 },
        ]}
        streaming={false}
        data-testid="trace"
      />
    );
    expect(restingLayer(screen.getByTestId('trace')).textContent).toBe('Thought for 1m 15s');
  });

  it('falls back to "Thought" for rows recorded before timing existed', () => {
    render(<ReasoningTrace texts={['plain text.']} streaming={false} data-testid="trace" />);
    expect(restingLayer(screen.getByTestId('trace')).textContent).toBe('Thought');
  });

  it('ticks the elapsed badge every second while streaming', () => {
    render(
      <ReasoningTrace
        texts={['**Planning the fix**\nworking']}
        timings={[{ startedAt: start, endedAt: start + 1_000 }]}
        streaming
        data-testid="trace"
      />
    );
    const badge = () =>
      screen.getByTestId('trace').querySelector('[data-slot="reasoning-panel-elapsed"]')
        ?.textContent;
    expect(badge()).toBe('10s');
    act(() => {
      vi.advanceTimersByTime(3_000);
    });
    expect(badge()).toBe('13s');
  });

  it('uses the newest heading as the live label, "Thinking" before any heading', () => {
    const { rerender } = render(
      <ReasoningTrace texts={['still working this out']} streaming data-testid="trace" />
    );
    const live = () =>
      screen.getByTestId('trace').querySelector('[data-slot="reasoning-panel-live-label"]')
        ?.textContent;
    expect(live()).toBe('Thinking');
    rerender(
      <ReasoningTrace
        texts={['**Reading**\nok\n**Planning the fix**\nnext']}
        streaming
        data-testid="trace"
      />
    );
    expect(live()).toBe('Planning the fix');
  });

  it('never renders the old hard-coded "Reasoning" label', () => {
    const { container } = render(
      <ReasoningTraceText text="**Plan**\nx" streaming={false} data-testid="trace" />
    );
    expect(container.textContent).not.toMatch(/\bReasoning\b/);
  });
});
