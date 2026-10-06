import { Infinity as InfinityIcon } from 'lucide-react';
import { useEffect, useState } from 'react';

import { useT } from '../../../lib/i18n/I18nContext';
import {
  openhumanGetAutonomySettings,
  openhumanUpdateAutonomySettings,
} from '../../../utils/tauriCommands/config';
import Button from '../../ui/Button';
import { SettingsNumberField, SettingsRow, SettingsSection, SettingsStatusLine } from '../controls';

// u32::MAX — the Rust default and our sentinel for "no limit". Inputs at or
// above this value render as "Unlimited" and clamp to UNLIMITED on save.
const UNLIMITED = 4_294_967_295;

/** Preset rows. The `label` field is an i18n key for the unlimited entry; the
 *  numeric-only rows are intentionally locale-agnostic. */
const UNLIMITED_ICON = <InfinityIcon className="h-3.5 w-3.5" aria-hidden />;

const PRESETS: { labelKey?: string; label?: string; value: number }[] = [
  { labelKey: 'autonomy.presetUnlimited', value: UNLIMITED },
  { label: '100', value: 100 },
  { label: '500', value: 500 },
  { label: '1000', value: 1000 },
];

const MIN = 1;
const MAX = UNLIMITED;

type Status =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'saving' }
  | { kind: 'saved' }
  | { kind: 'error'; message: string };

/**
 * Headerless section for editing the agent's max_actions_per_hour rate-limit,
 * rendered inside AgentAccessPanel (formerly the standalone /settings/autonomy
 * page — that slug now redirects to /settings/agent-access). Loads the current
 * value via openhumanGetAutonomySettings on mount; saving writes through
 * openhumanUpdateAutonomySettings and persists to the user's config.toml.
 * New value applies to the next agent session.
 */
const AutonomyRateLimitSection = () => {
  const { t } = useT();
  const [committed, setCommitted] = useState<number | null>(null);
  const [draft, setDraft] = useState<string>('');
  const [status, setStatus] = useState<Status>({ kind: 'loading' });

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await openhumanGetAutonomySettings();
        if (cancelled) return;
        const value = res.result.max_actions_per_hour;
        setCommitted(value);
        setDraft(String(value));
        setStatus({ kind: 'idle' });
      } catch (err) {
        if (cancelled) return;
        setStatus({ kind: 'error', message: err instanceof Error ? err.message : String(err) });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const trimmed = draft.trim();
  const parsed = Number(trimmed);
  const isValid =
    /^\d+$/.test(trimmed) && Number.isInteger(parsed) && parsed >= MIN && parsed <= MAX;
  const isChanged = committed !== null && parsed !== committed;
  const canSave = isValid && isChanged && status.kind !== 'saving';

  const applyPreset = (value: number) => {
    setDraft(String(value));
    if (status.kind === 'saved' || status.kind === 'error') {
      setStatus({ kind: 'idle' });
    }
  };

  const onSave = async () => {
    if (!canSave) return;
    setStatus({ kind: 'saving' });
    try {
      await openhumanUpdateAutonomySettings({ max_actions_per_hour: parsed });
      setCommitted(parsed);
      setStatus({ kind: 'saved' });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      // Revert UI to last committed value, then surface the error.
      if (committed !== null) setDraft(String(committed));
      setStatus({ kind: 'error', message });
    }
  };

  const savedNote = status.kind === 'saved' ? t('autonomy.statusSaved') : null;
  const errorMsg =
    status.kind === 'error' ? `${t('autonomy.statusFailed')}: ${status.message}` : null;

  return (
    <SettingsSection
      title={t('autonomy.maxActionsLabel')}
      description={t('autonomy.maxActionsHelp')}>
      <SettingsRow
        stacked
        control={
          <div className="space-y-3">
            <div className="flex items-center gap-2">
              {/* The u32::MAX sentinel shows as an empty field with an ∞
                  placeholder; clearing the field means unlimited again. */}
              <SettingsNumberField
                id="autonomy-max-actions"
                value={draft === String(UNLIMITED) ? '' : draft}
                placeholder="∞"
                rangeLabel={
                  <>
                    {MIN}&#x2013;
                    <InfinityIcon className="h-3 w-3" aria-label={t('autonomy.presetUnlimited')} />
                  </>
                }
                onChange={v => {
                  setDraft(v.trim() === '' ? String(UNLIMITED) : v);
                  if (status.kind === 'saved' || status.kind === 'error') {
                    setStatus({ kind: 'idle' });
                  }
                }}
                onCommit={() => {}}
                unit={t('autonomy.maxActionsLabel')}
                min={MIN}
                max={MAX}
                disabled={status.kind === 'loading' || status.kind === 'saving'}
                invalid={!isValid && trimmed !== ''}
                aria-label={t('autonomy.maxActionsLabel')}
              />
              <Button
                type="button"
                variant="primary"
                size="xs"
                data-testid="autonomy-max-actions-save"
                onClick={() => void onSave()}
                disabled={!canSave}>
                {status.kind === 'saving' ? t('autonomy.statusSaving') : t('common.save')}
              </Button>
            </div>

            <div className="flex flex-wrap gap-2">
              {PRESETS.map(p => (
                <Button
                  key={p.value}
                  type="button"
                  variant="tertiary"
                  size="xs"
                  leadingIcon={p.value === UNLIMITED ? UNLIMITED_ICON : undefined}
                  onClick={() => applyPreset(p.value)}>
                  {p.labelKey ? t(p.labelKey) : p.label}
                </Button>
              ))}
            </div>

            {!isValid && trimmed !== '' && (
              <p className="text-xs text-coral-600 dark:text-coral-300">
                {t('autonomy.invalidIntegerMsg')}
              </p>
            )}
            {isValid && parsed === UNLIMITED && (
              <p className="flex items-center gap-1.5 text-xs text-content-muted">
                {UNLIMITED_ICON}
                {t('autonomy.unlimitedNote')}
              </p>
            )}

            <SettingsStatusLine
              saving={status.kind === 'saving'}
              savedNote={savedNote}
              error={errorMsg}
              savingLabel={t('autonomy.statusSaving')}
            />
          </div>
        }
      />
    </SettingsSection>
  );
};

export default AutonomyRateLimitSection;
