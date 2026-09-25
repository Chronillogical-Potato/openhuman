import { useEffect, useMemo, useState } from 'react';

import { useCostDashboard } from '../../hooks/useCostDashboard';
import { useT } from '../../lib/i18n/I18nContext';
import { SettingsStatusLine } from '../settings/controls';
import SettingsPanel from '../settings/layout/SettingsPanel';
import { Alert, AlertDescription, Button, Card } from '../ui';
import CostBarChart from './CostBarChart';
import CostSummary from './CostSummary';
import DashboardSkeleton from './DashboardSkeleton';
import { relativeTime } from './formatCurrency';
import ModelCostTable from './ModelCostTable';
import TokenUsageChart from './TokenUsageChart';

interface CostDashboardPanelProps {
  /** When true the panel is hosted inside another settings page (e.g. the
   *  Usage tabs) — skip the standalone SettingsHeader chrome. */
  embedded?: boolean;
}

const CostDashboardPanel = ({ embedded = false }: CostDashboardPanelProps) => {
  const { t } = useT();
  const { data, isLoading, isFetching, error, lastUpdated, refetch } = useCostDashboard();

  const hasAnyUsage = useMemo(
    () => (data ? data.days.some(day => day.request_count > 0 || day.total_tokens > 0) : false),
    [data]
  );

  // Tick once a second so the "Updated Ns ago" pill stays fresh without
  // re-rendering the entire chart pipeline.
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = window.setInterval(() => setTick(n => n + 1), 1000);
    return () => window.clearInterval(id);
  }, []);

  const body = (
    <>
      <div className="flex items-start justify-between gap-3">
        <p className="text-xs text-content-muted max-w-prose">
          {t('settings.costDashboard.subtitle')}
        </p>
        <div className="flex items-center gap-2 shrink-0">
          {lastUpdated !== null && (
            <span
              data-testid="cost-dashboard-updated"
              className="inline-flex items-center gap-1.5 text-[11px] text-content-muted">
              <span
                aria-hidden
                className={`inline-block h-1.5 w-1.5 rounded-full ${isFetching ? 'bg-primary-500 animate-pulse' : 'bg-sage-500'}`}
              />
              {`${t('settings.costDashboard.updated')} ${relativeTime(lastUpdated ?? 0, t)}`}
            </span>
          )}
          <Button
            type="button"
            variant="secondary"
            size="xs"
            data-testid="cost-dashboard-refresh"
            onClick={() => void refetch()}
            disabled={isFetching}
            aria-label={t('settings.costDashboard.refresh')}
            leadingIcon={
              <RefreshIcon className={`h-3.5 w-3.5 ${isFetching ? 'animate-spin' : ''}`} />
            }>
            {t('settings.costDashboard.refresh')}
          </Button>
        </div>
      </div>

      {error && (
        <div role="alert" data-testid="cost-dashboard-error">
          <SettingsStatusLine saving={false} error={error} savingLabel="" />
        </div>
      )}
      {data && !data.enabled && (
        // A resolved config state, not a response to a user action — opt out
        // of the assertive default so it doesn't talk over the page it's on.
        <Alert
          variant="warning"
          density="compact"
          role={undefined}
          data-testid="cost-dashboard-disabled">
          <AlertDescription>{t('settings.costDashboard.disabledHint')}</AlertDescription>
        </Alert>
      )}

      {!data && isLoading && <DashboardSkeleton />}

      {data && (
        <>
          <CostSummary
            currency={data.currency}
            periodTotalUsd={data.period_total_usd}
            monthlyPaceUsd={data.monthly_pace_usd}
            monthToDateUsd={data.month_to_date_usd}
          />
          <Card
            data-testid="cost-dashboard-cost-chart"
            padded
            divided={false}
            className="bg-surface/40"
            title={t('settings.costDashboard.sevenDayCost')}
            headerRight={
              <span className="text-[11px] text-content-muted">
                {t('settings.costDashboard.utcNote')}
              </span>
            }>
            <CostBarChart days={data.days} currency={data.currency} />
          </Card>
          <Card
            data-testid="cost-dashboard-token-chart"
            padded
            divided={false}
            className="bg-surface/40"
            title={t('settings.costDashboard.sevenDayTokens')}
            headerRight={
              <span className="text-[11px] text-content-muted">
                {t('settings.costDashboard.stackedNote')}
              </span>
            }>
            <TokenUsageChart days={data.days} />
          </Card>
          <Card
            data-testid="cost-dashboard-model-table"
            padded
            divided={false}
            className="bg-surface/40"
            title={t('settings.costDashboard.modelBreakdown')}
            description={t('settings.costDashboard.modelBreakdownHint')}>
            <ModelCostTable models={data.by_model} currency={data.currency} />
          </Card>
          {!hasAnyUsage && (
            <div
              data-testid="cost-dashboard-empty"
              className="rounded-xl border border-dashed border-line-strong px-4 py-6 text-center">
              <div className="text-sm font-medium text-content-secondary">
                {t('settings.costDashboard.noData')}
              </div>
              <div className="text-[11px] text-content-muted mt-1">
                {t('settings.costDashboard.noDataHint')}
              </div>
            </div>
          )}
        </>
      )}
    </>
  );

  // Embedded inside the tabbed Usage page: the parent owns the header,
  // so render just the padded body.
  if (embedded)
    return (
      <div className="p-4 space-y-4" data-testid="cost-dashboard-panel">
        {body}
      </div>
    );

  return <SettingsPanel testId="cost-dashboard-panel">{body}</SettingsPanel>;
};

interface IconProps {
  className?: string;
}

const RefreshIcon = ({ className }: IconProps) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden>
    <polyline points="23 4 23 10 17 10" />
    <polyline points="1 20 1 14 7 14" />
    <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10" />
    <path d="M20.49 15a9 9 0 0 1-14.85 3.36L1 14" />
  </svg>
);

export default CostDashboardPanel;
