/**
 * Accessibility smoke lane (plan.md §7 — the frontend had zero a11y assertions).
 * Renders a few high-traffic, self-contained components and runs axe-core over
 * the output, asserting no violations. jsdom can't compute layout, so
 * color-contrast is auto-skipped; this catches the structural a11y bugs that
 * matter — missing roles/labels, invalid ARIA, unlabelled controls.
 *
 * Kept deliberately small and provider-light so it stays fast and stable; grow
 * it screen-by-screen rather than pulling in the full app shell.
 */
import { render } from '@testing-library/react';
import { axe } from 'jest-axe';
import { describe, expect, it, vi } from 'vitest';

import { ApprovalCardAdapter } from '../../features/conversations/aui/ApprovalCardAdapter';
import { ArtifactCardAdapter } from '../../features/conversations/aui/ArtifactCardAdapter';
import type { ArtifactSnapshot } from '../../store/chatRuntimeSlice';

vi.mock('../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

async function expectNoViolations(container: HTMLElement) {
  const results = await axe(container);
  expect(results.violations).toEqual([]);
}

describe('accessibility smoke', () => {
  it('ArtifactCardAdapter (ready) has no axe violations', async () => {
    const artifact: ArtifactSnapshot = {
      artifactId: 'a-1',
      kind: 'presentation',
      title: 'Quarterly Deck',
      status: 'ready',
      sizeBytes: 4096,
      path: 'a-1/deck.pptx',
      updatedAt: 0,
    };
    const { container } = render(<ArtifactCardAdapter artifact={artifact} />);
    await expectNoViolations(container);
  });

  it('ArtifactCardAdapter (failed with error) has no axe violations', async () => {
    const artifact: ArtifactSnapshot = {
      artifactId: 'a-2',
      kind: 'document',
      title: 'Report',
      status: 'failed',
      error: 'producer crashed',
      updatedAt: 0,
    };
    const { container } = render(<ArtifactCardAdapter artifact={artifact} onRetry={vi.fn()} />);
    await expectNoViolations(container);
  });

  it('ApprovalCardAdapter has no axe violations', async () => {
    const { container } = render(
      <ApprovalCardAdapter
        ariaLabel="Approval needed"
        title="Approval needed"
        subtitle="Run `shell` — shell (18 bytes of arguments)"
        command="pip show yfinance"
        toolName="shell"
        alwaysDecision="approve_always_for_tool"
        analyticsPrefix="chat-approval"
        onDecide={vi.fn()}
      />
    );
    await expectNoViolations(container);
  });
});
