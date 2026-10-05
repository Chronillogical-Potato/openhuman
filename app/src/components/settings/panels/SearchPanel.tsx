import { useEffect, useId, useState } from 'react';

import { useT } from '../../../lib/i18n/I18nContext';
import { useCoreState } from '../../../providers/CoreStateProvider';
import { isLocalSessionToken } from '../../../utils/localSession';
import {
  openhumanGetSearchSettings,
  openhumanUpdateSearchSettings,
  type SearchProviderUpdate,
  type SearchSettings,
  type SearchSettingsUpdate,
} from '../../../utils/tauriCommands/config';
import ChipTabs from '../../layout/ChipTabs';
import { Alert, AlertDescription } from '../../ui/Alert';
import { CenteredLoadingState } from '../../ui/LoadingState';
import StatusLine from '../../ui/StatusLine';
import Switch from '../../ui/Switch';
import SettingsTabbedPage from '../layout/SettingsTabbedPage';
import SearchPanelAllowedSites from './SearchPanelAllowedSites';
import SearchPanelProviders from './SearchPanelProviders';
import SearchPanelRoles from './SearchPanelRoles';

type Status =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'saving' }
  | { kind: 'saved' }
  | { kind: 'error'; message: string };

export type SearchPanelTab = 'providers' | 'routing' | 'websites';

const errorMessage = (err: unknown) => (err instanceof Error ? err.message : String(err));

/**
 * Web search settings, laid out like the LLM page: a page header carrying the
 * global on/off switch, then three chip tabs —
 *  - Providers: what is connected, and a catalogue of what can be added;
 *  - Routing: which provider serves each role (search / answer / contents);
 *  - Websites: the host allowlist for opening and reading pages.
 *
 * Everything is rendered from the core's `config_get_search_settings`
 * response. Every update returns the full settings object, which replaces
 * local state, so the view never guesses what the core decided (for example
 * which provider now serves a role).
 *
 * `embedded` renders without the page title (the onboarding wizard owns its
 * own heading); the switch and tabs then sit at the top of the body.
 */
const SearchPanel = ({
  embedded = false,
  hideTabChrome = false,
}: {
  embedded?: boolean;
  /**
   * Drop the providers/routing/websites chip tabs and show providers only.
   *
   * The onboarding wizard sets this: a three-step wizard with its own tabbed
   * sub-navigation inside one step reads as two nesting levels of the same
   * idea, and the routing and websites tabs are refinements nobody needs
   * before their first message. Both stay reachable from Settings. Defaults
   * false so Settings is unchanged.
   */
  hideTabChrome?: boolean;
}) => {
  const { t } = useT();
  const { snapshot } = useCoreState();
  const isLocalSession = isLocalSessionToken(snapshot.sessionToken);
  const enabledId = useId();

  const [selectedTab, setTab] = useState<SearchPanelTab>('providers');
  // With the chip row hidden there is no way back from another tab, so pin it.
  const tab: SearchPanelTab = hideTabChrome ? 'providers' : selectedTab;
  const [settings, setSettings] = useState<SearchSettings | null>(null);
  const [status, setStatus] = useState<Status>({ kind: 'loading' });
  const saving = status.kind === 'saving';

  useEffect(() => {
    let cancelled = false;
    openhumanGetSearchSettings()
      .then(res => {
        if (cancelled) return;
        setSettings(res.result);
        setStatus({ kind: 'idle' });
      })
      .catch(err => {
        if (cancelled) return;
        setStatus({ kind: 'error', message: errorMessage(err) });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  /** Send a patch; on success the returned settings replace local state. */
  const persist = async (update: SearchSettingsUpdate): Promise<boolean> => {
    if (!settings || saving) return false;
    setStatus({ kind: 'saving' });
    try {
      const res = await openhumanUpdateSearchSettings(update);
      if (res?.result) setSettings(res.result);
      setStatus({ kind: 'saved' });
      return true;
    } catch (err) {
      setStatus({ kind: 'error', message: errorMessage(err) });
      return false;
    }
  };

  const updateProvider = (id: string, patch: SearchProviderUpdate) =>
    persist({ providers: { [id]: patch } });

  const managedUnavailable = isLocalSession || (settings ? !settings.managed_available : false);

  const enabledSwitch = settings ? (
    <label
      htmlFor={enabledId}
      className="flex items-center gap-2.5 rounded-lg border border-line bg-surface px-3 py-1.5"
      data-testid="search-enabled">
      <span className="text-sm font-medium text-content">{t('settings.search.enabledLabel')}</span>
      <Switch
        id={enabledId}
        data-testid="search-enabled-toggle"
        aria-label={t('settings.search.enabledLabel')}
        checked={settings.enabled}
        disabled={saving}
        onCheckedChange={next => void persist({ enabled: next })}
      />
    </label>
  ) : null;

  const tabs = [
    { id: 'providers' as const, label: t('settings.search.tabProviders') },
    { id: 'routing' as const, label: t('settings.search.tabRouting') },
    { id: 'websites' as const, label: t('settings.search.tabWebsites') },
  ];

  // One honest answer to "will search actually work?". `effective_roles` lists
  // the usable providers per role in serving order, so an empty list for every
  // role means no tool will be offered to the agent. Derived from the settings
  // the panel already holds — `SearchProviderInfo.usable` is dead, nothing
  // reads it, and there is no RPC that runs a probe query.
  const searchReady = Boolean(
    settings?.enabled &&
    Object.values(settings.effective_roles ?? {}).some(providers => providers.length > 0)
  );

  const body = (
    <div className="flex w-full flex-col gap-4" data-testid="search-settings-panel">
      {hideTabChrome && settings && settings.enabled ? (
        <Alert
          variant={searchReady ? 'success' : 'warning'}
          density="compact"
          role={undefined}
          data-testid="search-readiness">
          <AlertDescription>
            {searchReady
              ? t('onboarding.custom.search.ready')
              : t('onboarding.custom.search.notReady')}
          </AlertDescription>
        </Alert>
      ) : null}

      {managedUnavailable && (
        <Alert variant="info">
          <AlertDescription>{t('settings.search.localManagedUnavailable')}</AlertDescription>
        </Alert>
      )}

      {settings && !settings.enabled && (
        <Alert variant="warning" data-testid="search-off-notice">
          <AlertDescription>{t('settings.search.offNotice')}</AlertDescription>
        </Alert>
      )}

      {status.kind === 'loading' && <CenteredLoadingState label={t('common.loading')} />}

      {settings && tab === 'providers' && (
        <SearchPanelProviders
          settings={settings}
          saving={saving}
          managedUnavailable={managedUnavailable}
          updateProvider={updateProvider}
          t={t}
        />
      )}
      {settings && tab === 'routing' && (
        <SearchPanelRoles settings={settings} saving={saving} persist={persist} t={t} />
      )}
      {settings && tab === 'websites' && (
        <SearchPanelAllowedSites settings={settings} saving={saving} persist={persist} t={t} />
      )}

      <StatusLine
        saving={saving}
        savedNote={status.kind === 'saved' ? t('settings.search.statusSaved') : null}
        error={
          status.kind === 'error' ? `${t('settings.search.statusError')}: ${status.message}` : null
        }
        savingLabel={t('settings.search.statusSaving')}
      />
    </div>
  );

  if (embedded) {
    return (
      <div className="flex w-full flex-col gap-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          {hideTabChrome ? null : (
            <ChipTabs
              className="flex flex-wrap gap-1.5"
              ariaLabel={t('settings.search.title')}
              testIdPrefix="search-tab"
              items={tabs}
              value={tab}
              onChange={setTab}
            />
          )}
          {enabledSwitch}
        </div>
        {body}
      </div>
    );
  }

  return (
    <SettingsTabbedPage
      title={t('settings.search.title')}
      description={t('connections.header.search')}
      headerAction={enabledSwitch}
      tabs={tabs}
      value={tab}
      onChange={setTab}
      tabsAriaLabel={t('settings.search.title')}
      tabsTestIdPrefix="search-tab">
      {body}
    </SettingsTabbedPage>
  );
};

export default SearchPanel;
