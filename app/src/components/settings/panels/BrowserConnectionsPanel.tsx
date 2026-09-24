import { useCallback, useEffect, useState } from 'react';
import { Link } from 'react-router-dom';

import { useT } from '../../../lib/i18n/I18nContext';
import { callCoreRpc } from '../../../services/coreRpcClient';
import {
  openhumanGetConfig,
  openhumanUpdateBrowserSettings,
} from '../../../utils/tauriCommands/config';
import SettingsTabbedPage from '../layout/SettingsTabbedPage';

type BrowserSettings = {
  enabled: boolean;
  headless: boolean;
  viewport_width: number;
  viewport_height: number;
  profile_mode: 'fresh' | 'persistent';
  max_task_steps: number;
  task_timeout_secs: number;
  chrome_path?: string | null;
  profile_path?: string | null;
  download_dir?: string | null;
};
type ModuleStatus = {
  id: string;
  state: 'available' | 'loading' | 'ready' | 'failed' | 'unsupported';
  detail?: string;
};

const numericBounds = {
  viewport_width: [320, 3840],
  viewport_height: [240, 2160],
  max_task_steps: [1, 100],
  task_timeout_secs: [5, 600],
} as const;

const defaults: BrowserSettings = {
  enabled: false,
  headless: true,
  viewport_width: 1280,
  viewport_height: 720,
  chrome_path: '',
  profile_mode: 'fresh',
  profile_path: '',
  download_dir: '',
  max_task_steps: 20,
  task_timeout_secs: 120,
};

export default function BrowserConnectionsPanel() {
  const { t } = useT();
  const [settings, setSettings] = useState<BrowserSettings>(defaults);
  const [module, setModule] = useState<ModuleStatus | null>(null);
  const [chromeReady, setChromeReady] = useState<boolean | null>(null);
  const [billingRoute, setBillingRoute] = useState<'direct_openrouter' | 'hosted' | null>(null);
  const [allowedDomains, setAllowedDomains] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');

  const refresh = useCallback(async () => {
    const [configResponse, moduleResponse] = await Promise.all([
      openhumanGetConfig(),
      callCoreRpc<{ result: { modules: ModuleStatus[] } }>({ method: 'openhuman.modules_list' }),
    ]);
    const config = configResponse.result.config;
    const browser = (config.browser ?? {}) as Partial<BrowserSettings>;
    setSettings({ ...defaults, ...browser });
    setBillingRoute(configResponse.result.browser_billing_route ?? null);
    const httpRequest = (config.http_request ?? {}) as { allowed_domains?: string[] };
    setAllowedDomains(httpRequest.allowed_domains ?? []);
    setModule(moduleResponse.result.modules.find(item => item.id === 'tinybrowser') ?? null);
  }, []);

  useEffect(() => {
    let active = true;
    void Promise.resolve().then(() => {
      if (!active) return;
      void refresh().catch(error => {
        if (active) setMessage(error instanceof Error ? error.message : String(error));
      });
    });
    return () => {
      active = false;
    };
  }, [refresh]);

  const save = async () => {
    setBusy(true);
    setMessage('');
    try {
      if (
        Object.entries(numericBounds).some(([key, [min, max]]) => {
          const value = settings[key as keyof typeof numericBounds];
          return !Number.isInteger(value) || value < min || value > max;
        })
      ) {
        setMessage(t('connections.browser.boundsRequired'));
        return;
      }
      await openhumanUpdateBrowserSettings(settings);
      setChromeReady(null);
      await refresh();
      setMessage(t('connections.browser.saved'));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  const testModule = async () => {
    setBusy(true);
    setMessage('');
    try {
      const response = await callCoreRpc<{ result: { module: ModuleStatus } }>({
        method: 'openhuman.modules_load',
        params: { id: 'tinybrowser' },
      });
      setModule(response.result.module);
      setMessage(response.result.module.detail ?? t('connections.browser.moduleChecked'));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  const testBrowser = async () => {
    setBusy(true);
    setMessage('');
    try {
      const response = await callCoreRpc<{
        result: { module_ready: boolean; chrome_ready: boolean; error?: string };
      }>({ method: 'openhuman.modules_browser_check_readiness' });
      setChromeReady(response.result.chrome_ready);
      if (response.result.module_ready) {
        setModule(current => (current ? { ...current, state: 'ready' } : current));
      }
      setMessage(response.result.error ?? t('connections.browser.readinessChecked'));
    } catch (error) {
      setChromeReady(false);
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  const field = (key: keyof BrowserSettings, label: string, type: 'text' | 'number' = 'text') => (
    <label className="flex flex-col gap-1 text-sm text-content" key={key}>
      <span>{label}</span>
      <input
        className="rounded-lg border border-line bg-surface px-3 py-2"
        type={type}
        min={type === 'number' ? numericBounds[key as keyof typeof numericBounds][0] : undefined}
        max={type === 'number' ? numericBounds[key as keyof typeof numericBounds][1] : undefined}
        value={settings[key] == null ? '' : String(settings[key])}
        onChange={event =>
          setSettings(current => ({
            ...current,
            [key]: type === 'number' ? Number(event.target.value) : event.target.value,
          }))
        }
      />
    </label>
  );

  const agenticRoute =
    billingRoute === 'direct_openrouter'
      ? t('connections.browser.routeDirect')
      : billingRoute === 'hosted'
        ? t('connections.browser.routeHosted')
        : t('connections.browser.unknown');

  return (
    <SettingsTabbedPage
      title={t('connections.tabs.browser')}
      description={t('connections.browser.description')}>
      <div className="max-w-3xl space-y-6 text-sm text-content">
        <div className="rounded-xl border border-line p-4 space-y-2">
          <p>
            {t('connections.browser.module')}:{' '}
            <strong>{module?.state ?? t('connections.browser.unknown')}</strong>
          </p>
          {module?.detail && <p className="text-content-muted">{module.detail}</p>}
          <p>
            {t('connections.browser.chrome')}:{' '}
            <strong>
              {chromeReady === true
                ? t('connections.browser.chromeReady')
                : chromeReady === false
                  ? t('connections.browser.chromeNotReady')
                  : t('connections.browser.notVerified')}
            </strong>
          </p>
          <p className="text-content-muted">{t('connections.browser.localOverride')}</p>
          <button
            className="rounded-lg border border-line px-3 py-2 disabled:opacity-50"
            disabled={busy}
            onClick={testModule}>
            {t('connections.browser.testModule')}
          </button>
          <button
            className="ml-2 rounded-lg border border-line px-3 py-2 disabled:opacity-50"
            disabled={busy}
            onClick={testBrowser}>
            {t('connections.browser.testBrowser')}
          </button>
        </div>
        <div className="grid gap-4 sm:grid-cols-2">
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={settings.enabled}
              onChange={event =>
                setSettings(current => ({ ...current, enabled: event.target.checked }))
              }
            />
            {t('connections.browser.enabled')}
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={settings.headless}
              onChange={event =>
                setSettings(current => ({ ...current, headless: event.target.checked }))
              }
            />
            {t('connections.browser.headless')}
          </label>
          {field('viewport_width', t('connections.browser.width'), 'number')}
          {field('viewport_height', t('connections.browser.height'), 'number')}
          {field('chrome_path', t('connections.browser.chromePath'))}
          <label className="flex flex-col gap-1">
            {t('connections.browser.profileMode')}
            <select
              className="rounded-lg border border-line bg-surface px-3 py-2"
              value={settings.profile_mode ?? 'fresh'}
              onChange={event =>
                setSettings(current => ({
                  ...current,
                  profile_mode: event.target.value as 'fresh' | 'persistent',
                }))
              }>
              <option value="fresh">{t('connections.browser.fresh')}</option>
              <option value="persistent">{t('connections.browser.persistent')}</option>
            </select>
          </label>
          {settings.profile_mode === 'persistent' &&
            field('profile_path', t('connections.browser.profilePath'))}
          {field('download_dir', t('connections.browser.downloadDir'))}
          {field('max_task_steps', t('connections.browser.maxSteps'), 'number')}
          {field('task_timeout_secs', t('connections.browser.timeout'), 'number')}
        </div>
        <div className="rounded-xl border border-line p-4 space-y-2">
          <p>{t('connections.browser.allowedWebsites')}</p>
          <p className="text-content-muted">{t('connections.browser.sharedPolicy')}</p>
          <p>
            {allowedDomains.length
              ? allowedDomains.join(', ')
              : t('connections.browser.noneAllowed')}
          </p>
          <Link className="text-ocean-600 underline" to="/connections?tab=search">
            {t('connections.browser.manageWebsites')}
          </Link>
        </div>
        <p>
          {t('connections.browser.jevRoute')}: <strong>{agenticRoute}</strong>
        </p>
        <p className="text-content-muted">{t('connections.browser.testInConversation')}</p>
        <button
          className="rounded-lg bg-ocean-600 px-4 py-2 text-content-inverted disabled:opacity-50"
          disabled={busy}
          onClick={save}>
          {t('connections.browser.save')}
        </button>
        {message && <p role="status">{message}</p>}
      </div>
    </SettingsTabbedPage>
  );
}
