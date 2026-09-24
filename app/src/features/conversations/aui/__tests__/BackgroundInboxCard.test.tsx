import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import type { BackgroundProcess } from '../../selectors/backgroundProcesses';
import { BackgroundInboxCard } from '../BackgroundInboxCard';

const procs: BackgroundProcess[] = [
  { taskId: 'sub-1', name: 'Researcher', goal: 'research the Eiffel Tower', status: 'running', toolCount: 16 },
  { taskId: 'sub-2', name: 'Archivist', goal: 'summarize notes', status: 'success', toolCount: 4 },
];

describe('BackgroundInboxCard', () => {
  it('renders nothing when closed', () => {
    render(
      <BackgroundInboxCard open={false} processes={procs} onClose={vi.fn()} onOpenProcess={vi.fn()} />
    );
    expect(document.body.querySelector('[data-testid="background-processes-panel"]')).toBeNull();
  });

  it('lists runs via the vendored BackgroundInbox and collects a settled one', async () => {
    const onOpenProcess = vi.fn();
    render(
      <BackgroundInboxCard open processes={procs} onClose={vi.fn()} onOpenProcess={onOpenProcess} />
    );
    expect(screen.getByText('Researcher')).toBeInTheDocument();
    expect(screen.getByText('Archivist')).toBeInTheDocument();

    // The running row is disabled (no collect); the settled one collects.
    await userEvent.click(screen.getByText('Archivist'));
    expect(onOpenProcess).toHaveBeenCalledWith('sub-2');
    expect(onOpenProcess).not.toHaveBeenCalledWith('sub-1');
  });

  it('shows the always-present scheduled + memory section scaffolding', () => {
    render(
      <BackgroundInboxCard open processes={procs} onClose={vi.fn()} onOpenProcess={vi.fn()} />
    );
    expect(screen.getByText('Scheduled jobs')).toBeInTheDocument();
    expect(screen.getByText('No scheduled jobs.')).toBeInTheDocument();
    expect(screen.getByText('Memory syncing')).toBeInTheDocument();
    expect(screen.getByText('All memories up to date')).toBeInTheDocument();
  });

  it('closes on Escape', () => {
    const onClose = vi.fn();
    render(
      <BackgroundInboxCard open processes={procs} onClose={onClose} onOpenProcess={vi.fn()} />
    );
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });
});
