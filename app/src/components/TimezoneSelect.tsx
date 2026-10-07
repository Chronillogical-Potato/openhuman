import { useEffect, useMemo, useState } from 'react';

import { useT } from '../lib/i18n/I18nContext';
import {
  openhumanGetUserTimezone,
  openhumanUpdateUserTimezone,
  type UserTimezoneSettings,
} from '../utils/tauriCommands';
import { SELECT_CLASS } from './LanguageSelect';

/** The option that follows the device's zone instead of a chosen one. */
const FOLLOW_DEVICE = '';

function ianaZones(): string[] {
  try {
    return Intl.supportedValuesOf('timeZone');
  } catch {
    return [];
  }
}

interface TimezoneSelectProps {
  /** Accessible label for the underlying <select>. */
  ariaLabel?: string;
}

/**
 * Settings → Account time zone picker. The user's IANA zone is what the
 * assistant reads dates in ("yesterday", "last Saturday"); "Use device time
 * zone" follows the machine instead. Loads and saves through core config
 * (`config_get_user_timezone` / `config_update_user_timezone`).
 */
const TimezoneSelect = ({ ariaLabel }: TimezoneSelectProps) => {
  const { t } = useT();
  const [settings, setSettings] = useState<UserTimezoneSettings | null>(null);
  const [failed, setFailed] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let live = true;
    openhumanGetUserTimezone()
      .then(response => {
        if (live) setSettings(response.result);
      })
      .catch(() => {
        if (live) setFailed(true);
      });
    return () => {
      live = false;
    };
  }, []);

  const zones = useMemo(() => {
    const listed = ianaZones();
    const chosen = settings?.timezone;
    return chosen && !listed.includes(chosen) ? [chosen, ...listed] : listed;
  }, [settings?.timezone]);

  const change = async (value: string) => {
    const timezone = value === FOLLOW_DEVICE ? null : value;
    const previous = settings;
    setSettings(current => (current ? { ...current, timezone } : current));
    setFailed(false);
    // The picker is disabled until this save settles, so two picks cannot
    // race and land out of order.
    setSaving(true);
    try {
      try {
        await openhumanUpdateUserTimezone(timezone);
      } catch {
        setSettings(previous);
        setFailed(true);
        return;
      }
      // Saved. A failed re-read leaves the chosen zone shown: it is stored.
      try {
        setSettings((await openhumanGetUserTimezone()).result);
      } catch {
        // keep the optimistic value
      }
    } finally {
      setSaving(false);
    }
  };

  const device = settings?.device ?? t('settings.timezoneUnknownDevice');
  return (
    <div className="flex flex-col items-end gap-1">
      <select
        value={settings?.timezone ?? FOLLOW_DEVICE}
        onChange={e => void change(e.target.value)}
        disabled={!settings || saving}
        aria-label={ariaLabel ?? t('settings.timezone')}
        data-testid="timezone-select"
        className={SELECT_CLASS}>
        <option value={FOLLOW_DEVICE}>
          {t('settings.timezoneDevice').replace('{zone}', device)}
        </option>
        {zones.map(zone => (
          <option key={zone} value={zone}>
            {zone.replace(/_/g, ' ')}
          </option>
        ))}
      </select>
      {failed && (
        <span
          role="alert"
          className="text-xs font-medium text-red-600"
          data-testid="timezone-error">
          {t('settings.timezoneSaveFailed')}
        </span>
      )}
    </div>
  );
};

export default TimezoneSelect;
