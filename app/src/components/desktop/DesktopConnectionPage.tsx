/** Local desktop automation setup in Connections. Permission truth comes from the core. */
import { useCallback, useEffect, useState } from 'react';

import { useT } from '../../lib/i18n/I18nContext';
import { callCoreRpc, isLocalDesktopHost } from '../../services/coreRpcClient';
import { openUrl } from '../../utils/openUrl';
import SettingsTabbedPage from '../settings/layout/SettingsTabbedPage';
import Button from '../ui/Button';
import Card from '../ui/Card';

export interface DesktopStatus {
  supported: boolean;
  enabled: boolean;
  platform: string;
  module_state: string;
  accessibility: string;
  screen_recording: string;
  jev_ready: boolean;
  reason?: string;
}

interface DesktopProbe {
  ok: boolean;
  app_count?: number;
  reason?: string;
}

interface DesktopPending {
  confirmation_id: string;
  app: string;
  operation: string;
  target_name: string | null;
  action_summary: string;
  reason: string;
  expires_at: string;
  approved: boolean;
}

type PermissionKind = 'accessibility' | 'screen_recording';

function permissionSettingsUrl(kind: PermissionKind, platform: string): string | null {
  if (platform === 'macos') {
    return `x-apple.systempreferences:com.apple.preference.security?Privacy_${kind === 'accessibility' ? 'Accessibility' : 'ScreenCapture'}`;
  }
  if (platform === 'windows') {
    return 'ms-settings:privacy';
  }
  return null;
}

export default function DesktopConnectionPage() {
  const { t } = useT();
  const [localHost, setLocalHost] = useState<boolean | null>(null);
  const [status, setStatus] = useState<DesktopStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingError, setPendingError] = useState<string | null>(null);
  const [probe, setProbe] = useState<DesktopProbe | null>(null);
  const [pending, setPending] = useState<DesktopPending[]>([]);

  useEffect(() => {
    let active = true;
    void isLocalDesktopHost().then(local => {
      if (active) setLocalHost(local);
    });
    return () => {
      active = false;
    };
  }, []);

  const refreshPending = useCallback(async () => {
    try {
      const entries = await callCoreRpc<DesktopPending[]>({ method: 'openhuman.desktop_pending' });
      setPending(entries.filter(entry => !entry.approved));
      setPendingError(null);
    } catch {
      setPendingError(t('desktop.pendingUnavailable'));
    }
  }, [t]);

  const refresh = useCallback(async () => {
    if (localHost === null) return;
    setLoading(true);
    setError(null);
    try {
      setStatus(
        localHost
          ? await callCoreRpc<DesktopStatus>({ method: 'openhuman.desktop_status' })
          : {
              supported: false,
              enabled: false,
              platform: 'remote',
              module_state: 'unavailable',
              accessibility: 'not_required',
              screen_recording: 'not_required',
              jev_ready: false,
            }
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [localHost]);

  useEffect(() => {
    void Promise.resolve().then(refresh);
  }, [refresh]);

  useEffect(() => {
    if (!status?.enabled) return;
    void Promise.resolve().then(refreshPending);
    const timer = window.setInterval(() => void refreshPending(), 5000);
    return () => window.clearInterval(timer);
  }, [status?.enabled, refreshPending]);

  const setEnabled = async () => {
    if (!localHost || !status || busy) return;
    setBusy(true);
    setError(null);
    setProbe(null);
    try {
      const next = await callCoreRpc<DesktopStatus>({
        method: 'openhuman.desktop_set_enabled',
        params: { enabled: !status.enabled },
      });
      setStatus(next);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };

  const runProbe = async () => {
    setBusy(true);
    setError(null);
    setProbe(null);
    try {
      setProbe(await callCoreRpc<DesktopProbe>({ method: 'openhuman.desktop_probe' }));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };

  const decide = async (confirmationId: string, approve: boolean) => {
    setBusy(true);
    setError(null);
    try {
      await callCoreRpc({
        method: 'openhuman.desktop_confirm',
        params: { confirmation_id: confirmationId, approve },
      });
      await refreshPending();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };

  const openSettings = async (kind: PermissionKind) => {
    if (!status) return;
    const url = permissionSettingsUrl(kind, status.platform);
    if (!url) return;
    try {
      await openUrl(url);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  const permissionRow = (kind: PermissionKind, value: string) => {
    const label = t(`desktop.permission.${kind}`);
    const url = status && permissionSettingsUrl(kind, status.platform);
    const stateLabel =
      value === 'granted'
        ? t('common.success')
        : value === 'denied'
          ? t('common.error')
          : t('common.notAvailable');
    return (
      <div key={kind} className="flex flex-wrap items-center justify-between gap-3 py-3">
        <div>
          <p className="text-sm font-medium text-content">{label}</p>
          <p className="text-xs text-content-muted">{stateLabel}</p>
        </div>
        {value !== 'granted' && value !== 'not_required' && url && (
          <Button variant="secondary" size="sm" onClick={() => void openSettings(kind)}>
            {t('desktop.openSettings')}
          </Button>
        )}
      </div>
    );
  };

  return (
    <SettingsTabbedPage
      title={t('desktop.title')}
      description={t('desktop.description')}
      headerAction={
        <Button
          variant="secondary"
          size="sm"
          onClick={() => {
            void refresh();
            if (status?.enabled) void refreshPending();
          }}
          disabled={loading}>
          {t('common.refresh')}
        </Button>
      }>
      <div className="max-w-2xl space-y-4" data-testid="desktop-connection-page">
        <Card title={t('desktop.title')} padded divided={false}>
          {loading && <p className="text-sm text-content-muted">{t('common.loading')}</p>}
          {!loading && status && (
            <div className="space-y-4">
              <p className="text-sm text-content-secondary">
                {!status.supported
                  ? t('desktop.unsupported')
                  : !status.enabled
                    ? t('common.disabled')
                    : status.module_state === 'failed'
                      ? t('desktop.moduleUnavailable')
                      : status.module_state === 'ready' &&
                          status.accessibility === 'granted' &&
                          status.jev_ready
                        ? t('channels.status.connected')
                        : t('desktop.enabledPending')}
              </p>
              {status.reason && <p className="text-sm text-content-muted">{status.reason}</p>}
              <p className="text-xs text-content-muted">{t('desktop.localOnly')}</p>
              {status.supported && (
                <Button onClick={() => void setEnabled()} disabled={busy}>
                  {status.enabled ? t('common.disable') : t('common.enable')}
                </Button>
              )}
            </div>
          )}
        </Card>

        {status?.supported && (
          <Card title={t('desktop.permissions')} padded>
            {permissionRow('accessibility', status.accessibility)}
            {permissionRow('screen_recording', status.screen_recording)}
            <p className="py-3 text-xs text-content-muted">{t('desktop.captureNote')}</p>
          </Card>
        )}

        {status?.supported && status.enabled && (
          <>
            {pending.length > 0 && (
              <Card title={t('chat.approval.title')} padded>
                {pending.map(entry => (
                  <div key={entry.confirmation_id} className="space-y-2 py-3">
                    <p className="text-sm font-medium text-content">
                      {entry.target_name
                        ? t('desktop.approvalSummary')
                            .replace(
                              '{operation}',
                              t(`desktop.action.${entry.operation.toLowerCase()}`, entry.operation)
                            )
                            .replace('{target}', entry.target_name)
                            .replace('{app}', entry.app)
                        : t('common.notAvailable')}
                    </p>
                    <p className="text-sm text-content-muted">{entry.reason}</p>
                    <div className="flex gap-2">
                      <Button
                        size="sm"
                        disabled={busy || !entry.action_summary || !entry.target_name}
                        onClick={() => void decide(entry.confirmation_id, true)}>
                        {t('chat.approval.approve')}
                      </Button>
                      <Button
                        size="sm"
                        variant="secondary"
                        disabled={busy}
                        onClick={() => void decide(entry.confirmation_id, false)}>
                        {t('chat.approval.deny')}
                      </Button>
                    </div>
                  </div>
                ))}
              </Card>
            )}
            <Card title={t('desktop.testButton')} padded divided={false}>
              <p className="mb-3 text-sm text-content-muted">{t('desktop.testDescription')}</p>
              <Button variant="secondary" onClick={() => void runProbe()} disabled={busy}>
                {t('desktop.testButton')}
              </Button>
              {probe && (
                <p className="mt-3 text-sm" role="status">
                  {probe.ok ? t('desktop.testPassed') : (probe.reason ?? t('desktop.testFailed'))}
                </p>
              )}
            </Card>
          </>
        )}

        {error && (
          <p className="text-sm text-coral-600 dark:text-coral-400" role="alert">
            {error}
          </p>
        )}
        {pendingError && (
          <p className="text-sm text-coral-600 dark:text-coral-400" role="alert">
            {pendingError}
          </p>
        )}
      </div>
    </SettingsTabbedPage>
  );
}
