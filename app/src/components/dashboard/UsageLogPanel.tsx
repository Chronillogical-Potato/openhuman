import { useMemo, useState } from 'react';

import { type CostUsageRecord, useCostUsageLog } from '../../hooks/useCostDashboard';
import { useT } from '../../lib/i18n/I18nContext';
import { SettingsStatusLine } from '../settings/controls';
import { Button, Card } from '../ui';
import { formatCurrency, formatTokens } from './formatCurrency';

const ALL = '';

/** Detailed local usage records. Filters apply to the fetched, bounded window. */
const UsageLogPanel = () => {
  const { t } = useT();
  const [days, setDays] = useState(30);
  const [category, setCategory] = useState(ALL);
  const [provider, setProvider] = useState(ALL);
  const [query, setQuery] = useState('');
  const [source, setSource] = useState(ALL);
  const { data, isLoading, isFetching, error, refetch } = useCostUsageLog({ days, limit: 1000 });

  const categories = useMemo(
    () => [...new Set(data?.records.map(record => record.category) ?? [])].sort(),
    [data]
  );
  const providers = useMemo(
    () => [...new Set(data?.records.map(record => record.provider ?? '') ?? [])].sort(),
    [data]
  );
  const records = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return (data?.records ?? []).filter(
      record =>
        (category === ALL || record.category === category) &&
        (provider === ALL || (record.provider ?? '') === provider) &&
        (source === ALL || record.cost_source === source) &&
        (!needle ||
          record.model.toLocaleLowerCase().includes(needle) ||
          record.session_id.toLocaleLowerCase().includes(needle))
    );
  }, [data, category, provider, source, query]);
  const filteredCost = records.reduce((sum, record) => sum + record.cost_usd, 0);

  return (
    <div className="p-4 space-y-4" data-testid="usage-log-panel">
      <div className="flex items-start justify-between gap-3">
        <p className="text-xs text-content-muted max-w-prose">
          {t('settings.costDashboard.usageLogHint')
            .replace('{days}', String(days))
            .replace('{limit}', '1000')}
        </p>
        <Button
          type="button"
          variant="secondary"
          size="xs"
          onClick={() => void refetch()}
          disabled={isFetching}>
          {t('settings.costDashboard.refresh')}
        </Button>
      </div>
      {error && <SettingsStatusLine saving={false} error={error} savingLabel="" />}
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-5">
        <Filter label={t('settings.costDashboard.period')}>
          <select
            value={days}
            onChange={event => setDays(Number(event.target.value))}
            className="w-full rounded-lg border border-line bg-surface px-2 py-1.5 text-xs">
            {[1, 7, 30, 90, 365].map(value => (
              <option key={value} value={value}>
                {t('settings.costDashboard.periodDays').replace('{days}', String(value))}
              </option>
            ))}
          </select>
        </Filter>
        <Filter label={t('settings.costDashboard.category')}>
          <select
            value={category}
            onChange={event => setCategory(event.target.value)}
            className="w-full rounded-lg border border-line bg-surface px-2 py-1.5 text-xs">
            <option value="">{t('settings.costDashboard.all')}</option>
            {categories.map(value => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </select>
        </Filter>
        <Filter label={t('settings.costDashboard.provider')}>
          <select
            value={provider}
            onChange={event => setProvider(event.target.value)}
            className="w-full rounded-lg border border-line bg-surface px-2 py-1.5 text-xs">
            <option value="">{t('settings.costDashboard.all')}</option>
            {providers.filter(Boolean).map(value => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </select>
        </Filter>
        <Filter label={t('settings.costDashboard.costSource')}>
          <select
            value={source}
            onChange={event => setSource(event.target.value)}
            className="w-full rounded-lg border border-line bg-surface px-2 py-1.5 text-xs">
            <option value="">{t('settings.costDashboard.all')}</option>
            <option value="estimated">{t('settings.costDashboard.estimated')}</option>
            <option value="provider_charged">{t('settings.costDashboard.providerCharged')}</option>
          </select>
        </Filter>
        <Filter label={t('settings.costDashboard.searchModelSession')}>
          <input
            value={query}
            onChange={event => setQuery(event.target.value)}
            className="w-full rounded-lg border border-line bg-surface px-2 py-1.5 text-xs"
          />
        </Filter>
      </div>
      <Card
        padded
        divided={false}
        className="bg-surface/40"
        title={t('settings.costDashboard.usageLog')}
        headerRight={
          data && (
            <span className="text-[11px] text-content-muted">
              {t('settings.costDashboard.filteredTotal')
                .replace('{shown}', String(records.length))
                .replace('{loaded}', String(data.records.length))
                .replace('{cost}', formatCurrency(filteredCost, data.currency))}
            </span>
          )
        }>
        {data ? (
          <UsageLogTable records={records} currency={data.currency} />
        ) : isLoading ? (
          <div className="text-xs text-content-muted">{t('settings.costDashboard.loading')}</div>
        ) : null}
      </Card>
    </div>
  );
};

const Filter = ({ label, children }: { label: string; children: React.ReactNode }) => (
  <label className="space-y-1 text-xs text-content-secondary">
    <span>{label}</span>
    {children}
  </label>
);

const UsageLogTable = ({ records, currency }: { records: CostUsageRecord[]; currency: string }) => {
  const { t } = useT();
  if (records.length === 0)
    return (
      <div className="text-xs text-content-muted italic py-2">
        {t('settings.costDashboard.noUsageLog')}
      </div>
    );
  return (
    <div className="overflow-x-auto -mx-1">
      <table className="w-full min-w-[850px] text-xs">
        <thead>
          <tr className="border-b border-line text-left text-[10px] uppercase tracking-wide text-content-muted">
            <th className="px-2 py-2">{t('settings.costDashboard.when')}</th>
            <th className="px-2 py-2">{t('settings.costDashboard.category')}</th>
            <th className="px-2 py-2">{t('settings.costDashboard.model')}</th>
            <th className="px-2 py-2 text-right">{t('settings.costDashboard.inputTokens')}</th>
            <th className="px-2 py-2 text-right">{t('settings.costDashboard.outputTokens')}</th>
            <th className="px-2 py-2 text-right">{t('settings.costDashboard.cost')}</th>
            <th className="px-2 py-2">{t('settings.costDashboard.costSource')}</th>
            <th className="px-2 py-2">{t('settings.costDashboard.session')}</th>
          </tr>
        </thead>
        <tbody>
          {records.map(record => (
            <tr
              key={record.id}
              className="border-b border-line-subtle last:border-0 hover:bg-surface-muted/60">
              <td className="px-2 py-2 whitespace-nowrap">{formatDateTime(record.timestamp)}</td>
              <td className="px-2 py-2">{record.category}</td>
              <td className="px-2 py-2">
                <div
                  className="max-w-[16rem] truncate font-medium text-content"
                  title={record.model}>
                  {record.model}
                </div>
                <div className="text-content-muted">
                  {record.provider ?? t('settings.costDashboard.unknownProvider')}
                </div>
              </td>
              <td className="px-2 py-2 text-right">{formatTokens(record.input_tokens)}</td>
              <td className="px-2 py-2 text-right">{formatTokens(record.output_tokens)}</td>
              <td className="px-2 py-2 text-right font-semibold">
                {formatCurrency(record.cost_usd, currency)}
              </td>
              <td className="px-2 py-2">
                {record.cost_source === 'provider_charged'
                  ? t('settings.costDashboard.providerCharged')
                  : t('settings.costDashboard.estimated')}
              </td>
              <td className="px-2 py-2 font-mono text-content-muted" title={record.session_id}>
                {record.session_id.slice(0, 8)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};

function formatDateTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  }).format(date);
}

export default UsageLogPanel;
