/**
 * Cron tool calls (`cron_add`, `cron_update`, `cron_list`) rendered through
 * the vendored `schedule-card` element instead of the raw JSON `ToolDataView`
 * fallback every other dynamic tool gets.
 *
 * The result shape is the core's own `CoreCronJob`
 * (`utils/tauriCommands/cron.ts`, already used by `CronJobsPanel`), so this
 * reads it directly rather than guessing at an undocumented shape. The
 * pause/resume switch calls the same `openhumanCronUpdate` RPC
 * `CronJobsPanel` uses and only flips its local `enabled` state after that
 * call resolves — a rendered tool call has no live subscription back to the
 * core's job list, so this is the point-in-time record of one `cron_*` call
 * plus a best-effort toggle on top of it, not a substitute for the Settings
 * panel's list.
 */
import { useCallback, useState } from 'react';

import { ScheduleCard, type ScheduleRun } from '../../../components/assistant-ui/elements/schedule-card';
import { useT } from '../../../lib/i18n/I18nContext';
import type { CoreCronJob, CoreCronRun } from '../../../utils/tauriCommands/cron';
import { openhumanCronUpdate } from '../../../utils/tauriCommands/cron';

function cadenceOf(job: CoreCronJob): string {
  if (job.schedule.kind === 'cron') return job.schedule.expr;
  if (job.schedule.kind === 'every') return `every ${Math.round(job.schedule.every_ms / 1000)}s`;
  return job.schedule.at;
}

function historyFromJob(job: CoreCronJob): ScheduleRun[] {
  if (!job.last_run) return [];
  return [{ id: `${job.id}:last`, at: job.last_run, ok: job.last_status !== 'error' }];
}

function historyFromRuns(runs: readonly CoreCronRun[]): ScheduleRun[] {
  return runs.map(run => ({ id: String(run.id), at: run.started_at, ok: run.status !== 'error' }));
}

function OneScheduleCard({ job, history }: { job: CoreCronJob; history: readonly ScheduleRun[] }) {
  const { t } = useT();
  const [enabled, setEnabled] = useState(job.enabled);
  const [busy, setBusy] = useState(false);

  const onToggle = useCallback(() => {
    if (busy) return;
    setBusy(true);
    void openhumanCronUpdate(job.id, { enabled: !enabled })
      .then(() => setEnabled(previous => !previous))
      .catch(() => {
        // Best-effort: the Settings panel is the authoritative surface for
        // cron errors (`CronJobsPanel`'s own `coreError` state); a failed
        // toggle from a historical tool-call render simply does not flip.
      })
      .finally(() => setBusy(false));
  }, [busy, enabled, job.id]);

  return (
    <ScheduleCard
      name={job.name ?? job.command}
      cadence={cadenceOf(job)}
      nextRun={job.next_run}
      enabled={enabled}
      history={history}
      onToggle={onToggle}
      nextLabel={t('conversations.scheduleCard.next')}
      pausedLabel={t('conversations.scheduleCard.paused')}
      recentRunsLabel={t('conversations.scheduleCard.recentRuns')}
      okLabel={t('conversations.scheduleCard.ok')}
      failedLabel={t('conversations.scheduleCard.failed')}
    />
  );
}

function isCoreCronJob(value: unknown): value is CoreCronJob {
  return !!value && typeof value === 'object' && 'id' in value && 'schedule' in value;
}

/** `cron_add` / `cron_update`: the single job the call returned. */
export function CronAddOrUpdateCall({ result }: { result: unknown }) {
  if (!isCoreCronJob(result)) return null;
  return <OneScheduleCard job={result} history={historyFromJob(result)} />;
}

/** `cron_list`: every job the call returned, most-imminent first. */
export function CronListCall({ result }: { result: unknown }) {
  const jobs = Array.isArray(result) ? result.filter(isCoreCronJob) : [];
  if (jobs.length === 0) return null;
  return (
    <div className="flex flex-col gap-2">
      {jobs.map(job => (
        <OneScheduleCard key={job.id} job={job} history={historyFromJob(job)} />
      ))}
    </div>
  );
}

/** `cron_runs`: one job's run history, read from `args.job_id` + the result list. */
export function CronRunsCall({ args, result }: { args: unknown; result: unknown }) {
  const jobId = args && typeof args === 'object' ? (args as { job_id?: unknown }).job_id : undefined;
  const runs = Array.isArray(result) ? result.filter((r): r is CoreCronRun => !!r && typeof r === 'object') : [];
  if (typeof jobId !== 'string' || runs.length === 0) return null;
  return (
    <OneScheduleCard
      job={{
        id: jobId,
        expression: '',
        schedule: { kind: 'cron', expr: '' },
        command: jobId,
        job_type: 'shell',
        session_target: 'isolated',
        enabled: true,
        delivery: { mode: 'none', best_effort: true },
        delete_after_run: false,
        created_at: '',
        next_run: '',
      }}
      history={historyFromRuns(runs)}
    />
  );
}
