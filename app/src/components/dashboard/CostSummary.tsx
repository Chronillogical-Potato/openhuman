import type { ReactNode } from 'react';

import { useT } from '../../lib/i18n/I18nContext';
import { formatCurrency } from './formatCurrency';

interface CostSummaryProps {
  currency: string;
  periodTotalUsd: number;
  monthlyPaceUsd: number;
  monthToDateUsd: number;
}

const CostSummary = ({
  currency,
  periodTotalUsd,
  monthlyPaceUsd,
  monthToDateUsd,
}: CostSummaryProps) => {
  const { t } = useT();

  return (
    <section
      data-testid="cost-dashboard-summary"
      className="grid grid-cols-1 md:grid-cols-2 gap-3"
      aria-label={t('settings.costDashboard.summaryAriaLabel')}>
      <div
        data-testid="metric-total-spend"
        className="rounded-2xl border border-line bg-linear-to-br from-primary-50 to-surface dark:from-neutral-900 dark:to-neutral-950 p-5 flex flex-col gap-3 shadow-soft">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2 text-xs uppercase tracking-wide text-content-muted">
            <WalletIcon className="h-4 w-4" />
            <span>{t('settings.costDashboard.totalSpend')}</span>
          </div>
        </div>
        <div className="flex items-baseline gap-3">
          <span className="text-3xl md:text-4xl font-semibold tabular-nums text-content">
            {formatCurrency(periodTotalUsd, currency)}
          </span>
          <span className="text-xs text-content-muted">
            {t('settings.costDashboard.lastSevenDays')}
          </span>
        </div>
        <div className="text-xs text-content-muted">
          {`${t('settings.costDashboard.monthToDate')}: ${formatCurrency(monthToDateUsd, currency)}`}
        </div>
      </div>

      <div className="grid grid-cols-1 gap-3">
        <SmallMetric
          icon={<TrendingUpIcon className="h-4 w-4" />}
          label={t('settings.costDashboard.monthlyPace')}
          value={formatCurrency(monthlyPaceUsd, currency)}
          hint={t('settings.costDashboard.monthlyPaceHint')}
          testId="metric-monthly-pace"
        />
      </div>
    </section>
  );
};

interface SmallMetricProps {
  icon: ReactNode;
  label: string;
  value: string;
  hint: string;
  testId: string;
}

const SmallMetric = ({ icon, label, value, hint, testId }: SmallMetricProps) => (
  <div
    data-testid={testId}
    title={hint}
    className="rounded-2xl border border-line p-3 flex flex-col gap-1 hover:border-primary-300 dark:hover:border-primary-700 transition-colors">
    <div className="flex items-center gap-2 text-[11px] uppercase tracking-wide text-content-muted">
      {icon}
      <span>{label}</span>
    </div>
    <span className="text-lg font-semibold tabular-nums text-content">{value}</span>
  </div>
);

interface IconProps {
  className?: string;
}

const WalletIcon = ({ className }: IconProps) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden>
    <path d="M21 12V7H5a2 2 0 0 1 0-4h14v4" />
    <path d="M3 5v14a2 2 0 0 0 2 2h16v-5" />
    <circle cx="17" cy="14" r="1.5" />
  </svg>
);

const TrendingUpIcon = ({ className }: IconProps) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden>
    <polyline points="23 6 13.5 15.5 8.5 10.5 1 18" />
    <polyline points="17 6 23 6 23 12" />
  </svg>
);

export default CostSummary;
