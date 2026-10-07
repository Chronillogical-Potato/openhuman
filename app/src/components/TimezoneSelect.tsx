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

function isAreaZone(zone: string | null | undefined): zone is string {
  return !!zone && (zone === 'UTC' || zone.includes('/'));
}

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

  // Only an Area/Location zone (or UTC) is ever offered; a saved value that is
  // not one shows as "follow the device", which is what core resolves it to.
  const chosen = isAreaZone(settings?.timezone) ? settings?.timezone : null;
  const zones = useMemo(() => {
    const listed = ianaZones();
    return chosen && !listed.includes(chosen) ? [chosen, ...listed] : listed;
  }, [chosen]);

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
      // Saved: show exactly what was stored. No re-read, so a late or
      // stale read can never put the previous zone back on screen.
      setSettings(current =>
        current ? { ...current, timezone, effective: timezone ?? current.device ?? 'UTC' } : current
      );
    } finally {
      setSaving(false);
    }
  };

  const device = settings?.device ?? t('settings.timezoneUnknownDevice');
  return (
    <div className="flex flex-col items-end gap-1">
      <select
        value={chosen ?? FOLLOW_DEVICE}
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
