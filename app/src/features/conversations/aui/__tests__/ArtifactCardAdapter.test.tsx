import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import type { ArtifactSnapshot } from '../../../../store/chatRuntimeSlice';
import { ArtifactCardAdapter } from '../ArtifactCardAdapter';

describe('ArtifactCardAdapter', () => {
  it('renders the generating (in_progress) state', () => {
    const artifact: ArtifactSnapshot = {
      artifactId: 'a-1',
      kind: 'document',
      title: 'Report',
      status: 'in_progress',
      updatedAt: 0,
    };
    render(<ArtifactCardAdapter artifact={artifact} />);
    expect(screen.getByText('Report')).toBeInTheDocument();
  });

  it('renders the ready state with a formatted size', () => {
    const artifact: ArtifactSnapshot = {
      artifactId: 'a-1',
      kind: 'presentation',
      title: 'Quarterly Deck',
      status: 'ready',
      sizeBytes: 4096,
      path: 'a-1/deck.pptx',
      updatedAt: 0,
    };
    render(<ArtifactCardAdapter artifact={artifact} />);
    expect(screen.getByText('Quarterly Deck')).toBeInTheDocument();
  });

  it('renders a Retry action for a failed artifact and calls onRetry with its id', async () => {
    const artifact: ArtifactSnapshot = {
      artifactId: 'a-2',
      kind: 'document',
      title: 'Report',
      status: 'failed',
      error: 'producer crashed',
      updatedAt: 0,
    };
    const onRetry = vi.fn();
    render(<ArtifactCardAdapter artifact={artifact} onRetry={onRetry} />);
    const retry = screen.getByRole('button');
    await userEvent.click(retry);
    expect(onRetry).toHaveBeenCalledWith('a-2');
  });

  it('renders no Retry action for a failed artifact when onRetry is omitted', () => {
    const artifact: ArtifactSnapshot = {
      artifactId: 'a-2',
      kind: 'document',
      title: 'Report',
      status: 'failed',
      error: 'producer crashed',
      updatedAt: 0,
    };
    render(<ArtifactCardAdapter artifact={artifact} />);
    expect(screen.queryByRole('button')).toBeNull();
  });
});
