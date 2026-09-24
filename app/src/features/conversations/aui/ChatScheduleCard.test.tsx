import type { ToolCallMessagePartProps } from '@assistant-ui/react';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { CoreCronJob } from '../../../utils/tauriCommands/cron';
import * as cron from '../../../utils/tauriCommands/cron';
import { CronAddOrUpdateCall, CronListCall, CronRunsCall } from './ChatScheduleCard';

function toolCallProps(toolName: string, args: unknown, result: unknown): ToolCallMessagePartProps {
  return {
    type: 'tool-call',
    toolName,
    toolCallId: `${toolName}-1`,
    args: args as never,
    argsText: '{}',
    result,
    status: { type: 'complete' },
    addResult: () => {},
    resume: () => {},
    respondToApproval: () => Promise.resolve(),
  };
}

function job(overrides: Partial<CoreCronJob> = {}): CoreCronJob {
  return {
    id: 'job-1',
    expression: '0 9 * * *',
    schedule: { kind: 'cron', expr: '0 9 * * *' },
    command: 'daily-report',
    name: 'Daily report',
    job_type: 'agent',
    session_target: 'isolated',
    enabled: true,
    delivery: { mode: 'none', best_effort: true },
    delete_after_run: false,
    created_at: '2026-01-01T00:00:00.000Z',
    next_run: '2026-01-02T09:00:00.000Z',
    last_run: '2026-01-01T09:00:00.000Z',
    last_status: 'ok',
    ...overrides,
  };
}

describe('cron tool call renders', () => {
  it('CronAddOrUpdateCall renders the vendored schedule-card for a single job', () => {
    render(<CronAddOrUpdateCall {...toolCallProps('cron_add', {}, job())} />);
    expect(screen.getByText('Daily report')).toBeTruthy();
    expect(screen.getByText('0 9 * * *')).toBeTruthy();
  });

  it('CronAddOrUpdateCall renders nothing for a non-job result', () => {
    const { container } = render(<CronAddOrUpdateCall {...toolCallProps('cron_add', {}, null)} />);
    expect(container.querySelector('[data-slot="schedule-card"]')).toBeNull();
  });

  it('CronListCall renders one card per job', () => {
    render(<CronListCall {...toolCallProps('cron_list', {}, [job(), job({ id: 'job-2', name: 'Weekly digest' })])} />);
    expect(screen.getByText('Daily report')).toBeTruthy();
    expect(screen.getByText('Weekly digest')).toBeTruthy();
  });

  it('CronRunsCall renders the run history keyed by args.job_id', () => {
    render(
      <CronRunsCall
        {...toolCallProps('cron_runs', { job_id: 'job-1' }, [
          { id: 1, job_id: 'job-1', started_at: '2026-01-01T09:00:00.000Z', finished_at: '', status: 'ok' },
        ])}
      />
    );
    expect(screen.getByText('2026-01-01T09:00:00.000Z')).toBeTruthy();
  });

  it('toggling the switch calls the cron update RPC and flips only after it resolves', async () => {
    const spy = vi.spyOn(cron, 'openhumanCronUpdate').mockResolvedValue({ result: job({ enabled: false }) });
    render(<CronAddOrUpdateCall {...toolCallProps('cron_add', {}, job())} />);

    const toggle = screen.getByRole('switch');
    expect(toggle).toHaveAttribute('aria-checked', 'true');
    toggle.click();

    expect(spy).toHaveBeenCalledWith('job-1', { enabled: false });
    await vi.waitFor(() => expect(toggle).toHaveAttribute('aria-checked', 'false'));
  });
});
