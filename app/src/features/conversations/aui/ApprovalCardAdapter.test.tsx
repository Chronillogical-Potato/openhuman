import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { ApprovalCardAdapter } from './ApprovalCardAdapter';

describe('ApprovalCardAdapter', () => {
  it('renders the title, subtitle and command', () => {
    render(
      <ApprovalCardAdapter
        ariaLabel="Approval needed"
        title="Approval needed"
        subtitle="Run `shell` — list files"
        command="ls -la"
        toolName="shell"
        analyticsPrefix="chat-approval"
        onDecide={vi.fn()}
      />
    );

    expect(screen.getByText('Approval needed')).toBeInTheDocument();
    expect(screen.getByText('Run `shell` — list files')).toBeInTheDocument();
    expect(screen.getByText('ls -la')).toBeInTheDocument();
  });

  it('omits the always-allow button when no alwaysDecision is supplied', () => {
    render(
      <ApprovalCardAdapter
        ariaLabel="Approval needed"
        title="Approval needed"
        subtitle="x"
        command="x"
        toolName="shell"
        analyticsPrefix="unrouted-approval"
        onDecide={vi.fn()}
      />
    );

    expect(screen.queryByRole('button', { name: 'Always allow' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Approve' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Deny' })).toBeInTheDocument();
  });

  it('calls onDecide with the option id for each button', async () => {
    const onDecide = vi.fn().mockResolvedValue(undefined);
    render(
      <ApprovalCardAdapter
        ariaLabel="Approval needed"
        title="Approval needed"
        subtitle="x"
        command="x"
        toolName="shell"
        alwaysDecision="approve_always_for_tool"
        analyticsPrefix="chat-approval"
        onDecide={onDecide}
      />
    );

    await userEvent.click(screen.getByRole('button', { name: 'Deny' }));
    expect(onDecide).toHaveBeenCalledWith('deny');
  });

  it('shows the error message and re-enables the buttons when onDecide rejects', async () => {
    const onDecide = vi.fn().mockRejectedValue(new Error('boom'));
    render(
      <ApprovalCardAdapter
        ariaLabel="Approval needed"
        title="Approval needed"
        subtitle="x"
        command="x"
        toolName="shell"
        analyticsPrefix="chat-approval"
        onDecide={onDecide}
      />
    );

    await userEvent.click(screen.getByRole('button', { name: 'Approve' }));

    await waitFor(() =>
      expect(screen.getByText(/Could not record your decision/)).toBeInTheDocument()
    );
    expect(screen.getByRole('button', { name: 'Approve' })).toBeEnabled();
  });

  it('disables every button while an external busy flag is set', () => {
    render(
      <ApprovalCardAdapter
        ariaLabel="Approval needed"
        title="Approval needed"
        subtitle="x"
        command="x"
        toolName="shell"
        analyticsPrefix="chat-approval"
        onDecide={vi.fn()}
        busy
      />
    );

    expect(screen.getByRole('button', { name: 'Approve' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Deny' })).toBeDisabled();
  });

  it('shows a live expiry countdown when expiresAt is in the future', () => {
    vi.useFakeTimers().setSystemTime(new Date('2026-01-01T00:00:00Z'));
    render(
      <ApprovalCardAdapter
        ariaLabel="Approval needed"
        title="Approval needed"
        subtitle="x"
        command="x"
        toolName="shell"
        expiresAt="2026-01-01T00:01:05Z"
        analyticsPrefix="chat-approval"
        onDecide={vi.fn()}
      />
    );

    expect(screen.getByText(/Expires in 1:05/)).toBeInTheDocument();
    vi.useRealTimers();
  });

  it('shows no countdown once the request has already expired', () => {
    vi.useFakeTimers().setSystemTime(new Date('2026-01-01T00:05:00Z'));
    render(
      <ApprovalCardAdapter
        ariaLabel="Approval needed"
        title="Approval needed"
        subtitle="x"
        command="x"
        toolName="shell"
        expiresAt="2026-01-01T00:01:00Z"
        analyticsPrefix="chat-approval"
        onDecide={vi.fn()}
      />
    );

    expect(screen.queryByText(/Expires in/)).not.toBeInTheDocument();
    vi.useRealTimers();
  });
});
