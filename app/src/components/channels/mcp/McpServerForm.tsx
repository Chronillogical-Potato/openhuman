/**
 * Add or edit one MCP server through a form — the `claude mcp add` shape.
 *
 * Name, how it runs (a local command or a remote URL), what it needs
 * (environment variables for a command, an authentication method for a URL),
 * and Save. The form writes the same `mcp.json` document the editor tab does:
 * it reads the current document, sets or replaces this one entry, and writes
 * the whole thing back through `config_set`. There is no second store.
 *
 * Authentication for a remote server is one of: none, a bearer token (sent as
 * `Authorization: Bearer …`), a custom header, or browser sign-in (OAuth). The
 * first three are stored as write-only headers. Sign-in cannot be stored ahead
 * of time — it needs the server's own consent page — so the form saves the
 * server and then hands the caller its name to open the connect dialog, which
 * runs the sign-in and reconnects.
 *
 * Editing never shows a stored value. The form lists the credential *names*
 * that are stored with empty value fields; a field left blank keeps what is
 * stored, a value replaces it, and a removed row clears it.
 */
import { Plus, X } from 'lucide-react';
import { type FormEvent, useCallback, useId, useMemo, useState } from 'react';

import { useT } from '../../../lib/i18n/I18nContext';
import { mcpClientsApi } from '../../../services/api/mcpClientsApi';
import { Alert, AlertDescription } from '../../ui';
import Button from '../../ui/Button';
import Label from '../../ui/Label';
import { ModalShell } from '../../ui/ModalShell';
import NativeSelect from '../../ui/NativeSelect';
import TextField from '../../ui/TextField';
import type { InstalledServer, McpConfigEntry } from './types';

type Transport = 'stdio' | 'http';
type AuthMode = 'none' | 'bearer' | 'header' | 'oauth';

interface Pair {
  key: string;
  value: string;
  /** A name the store already holds; a blank value keeps it. */
  stored: boolean;
}

interface McpServerFormProps {
  /** Present when editing; absent when adding. */
  existing?: InstalledServer;
  onClose: () => void;
  /**
   * The server was written. `signIn` asks the caller to open the connect
   * dialog for it, because the user chose browser sign-in.
   */
  onSaved: (name: string, opts: { signIn: boolean }) => void;
}

/** Split a typed argument line the way a shell would, honouring quotes. */
export const splitArgs = (raw: string): string[] => {
  const out: string[] = [];
  const re = /"([^"]*)"|'([^']*)'|(\S+)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(raw)) !== null) out.push(m[1] ?? m[2] ?? m[3] ?? '');
  return out;
};

const pairsToRecord = (pairs: Pair[]): Record<string, string> => {
  const out: Record<string, string> = {};
  for (const { key, value } of pairs) {
    const k = key.trim();
    if (!k) continue;
    // A stored name left blank is not sent: the core keeps the stored value.
    if (value === '') continue;
    out[k] = value;
  }
  return out;
};

const McpServerForm = ({ existing, onClose, onSaved }: McpServerFormProps) => {
  const { t } = useT();
  const titleId = useId();
  const editing = existing !== undefined;
  const storedKeys = useMemo(
    () => (existing?.env_keys ?? []).filter(k => !k.startsWith('__')),
    [existing]
  );
  const hostedExisting = existing?.transport?.kind === 'http_remote';

  const [name, setName] = useState(existing?.qualified_name ?? '');
  const [transport, setTransport] = useState<Transport>(hostedExisting ? 'http' : 'stdio');
  const [command, setCommand] = useState(existing && !hostedExisting ? existing.command : '');
  const [args, setArgs] = useState(
    existing && !hostedExisting
      ? existing.args.map(a => (/\s/.test(a) ? `"${a}"` : a)).join(' ')
      : ''
  );
  const [url, setUrl] = useState(hostedExisting ? (existing.transport as { url: string }).url : '');
  const [env, setEnv] = useState<Pair[]>(() =>
    !hostedExisting && storedKeys.length
      ? storedKeys.map(key => ({ key, value: '', stored: true }))
      : [{ key: '', value: '', stored: false }]
  );
  // A stored `Authorization` reads as a bearer token; any other stored header
  // as a custom one. Nothing stored means the user has not chosen yet.
  const [authMode, setAuthMode] = useState<AuthMode>(() => {
    if (!hostedExisting) return 'none';
    if (storedKeys.some(k => k.toLowerCase() === 'authorization')) {
      return existing?.env_keys.includes('__oauth__') ? 'oauth' : 'bearer';
    }
    return storedKeys.length ? 'header' : 'none';
  });
  const [token, setToken] = useState('');
  const [headerName, setHeaderName] = useState(
    hostedExisting ? (storedKeys.find(k => k.toLowerCase() !== 'authorization') ?? '') : ''
  );
  const [headerValue, setHeaderValue] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const nameValid = /^[A-Za-z0-9][A-Za-z0-9._@\/-]*$/.test(name.trim());
  const canSave =
    !saving &&
    nameValid &&
    (transport === 'stdio'
      ? command.trim().length > 0
      : /^https?:\/\/\S+$/.test(url.trim()) &&
        (authMode !== 'header' || headerName.trim().length > 0));

  const updateEnv = (index: number, patch: Partial<Pair>) =>
    setEnv(prev => prev.map((p, i) => (i === index ? { ...p, ...patch } : p)));

  const handleSubmit = useCallback(
    async (event: FormEvent) => {
      event.preventDefault();
      if (!canSave) return;
      setSaving(true);
      setError(null);
      const trimmedName = name.trim();
      try {
        const entry: McpConfigEntry = {};
        if (transport === 'stdio') {
          entry.command = command.trim();
          const parsed = splitArgs(args);
          if (parsed.length) entry.args = parsed;
          // Names the user removed from the list are cleared on the core.
          const written = pairsToRecord(env);
          for (const key of storedKeys) {
            if (!env.some(p => p.key.trim() === key)) written[key] = '';
          }
          if (Object.keys(written).length) entry.env = written;
        } else {
          entry.url = url.trim();
          const headers: Record<string, string> = {};
          const clearOthers = (keep: string | null) => {
            for (const key of storedKeys) {
              if (keep === null || key.toLowerCase() !== keep.toLowerCase()) headers[key] = '';
            }
          };
          if (authMode === 'bearer') {
            clearOthers('Authorization');
            if (token.trim()) headers.Authorization = `Bearer ${token.trim()}`;
          } else if (authMode === 'header') {
            clearOthers(headerName.trim());
            if (headerValue.trim()) headers[headerName.trim()] = headerValue.trim();
          } else if (authMode === 'none') {
            clearOthers(null);
          }
          // OAuth: leave whatever the sign-in stored alone.
          if (Object.keys(headers).length) entry.headers = headers;
        }
        if (existing && !existing.enabled) entry.enabled = false;
        if (existing?.description) entry.description = existing.description;

        const doc = await mcpClientsApi.configGet();
        const next = { ...doc.mcpServers };
        if (existing && existing.qualified_name !== trimmedName) {
          delete next[existing.qualified_name];
        }
        next[trimmedName] = entry;
        await mcpClientsApi.configSet({ mcpServers: next });
        onSaved(trimmedName, { signIn: transport === 'http' && authMode === 'oauth' });
      } catch (err) {
        setError(err instanceof Error ? err.message : t('mcp.form.saveFailed'));
        setSaving(false);
      }
    },
    [
      canSave,
      name,
      transport,
      command,
      args,
      env,
      storedKeys,
      url,
      authMode,
      token,
      headerName,
      headerValue,
      existing,
      onSaved,
      t,
    ]
  );

  const formId = `${titleId}-form`;

  return (
    <ModalShell
      onClose={onClose}
      titleId={titleId}
      title={editing ? t('mcp.form.editTitle').replace('{name}', existing.display_name) : t('mcp.form.addTitle')}
      subtitle={t('mcp.form.subtitle')}
      maxWidthClassName="max-w-lg"
      contentClassName="space-y-4 p-5"
      testId="mcp-server-form"
      closePolicy={saving ? { escape: false, backdrop: false, button: false } : undefined}
      footer={
        <div className="flex items-center justify-end gap-2">
          <Button variant="secondary" size="sm" disabled={saving} onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button
            type="submit"
            form={formId}
            variant="primary"
            size="sm"
            disabled={!canSave}
            data-testid="mcp-form-save">
            {saving
              ? t('mcp.form.saving')
              : transport === 'http' && authMode === 'oauth'
                ? t('mcp.form.saveAndSignIn')
                : editing
                  ? t('mcp.form.save')
                  : t('mcp.form.add')}
          </Button>
        </div>
      }>
      <form id={formId} className="space-y-4" onSubmit={e => void handleSubmit(e)}>
        {/* Name */}
        <div className="space-y-1">
          <Label htmlFor={`${titleId}-name`}>{t('mcp.form.name')}</Label>
          <TextField
            id={`${titleId}-name`}
            mono
            inputSize="sm"
            value={name}
            onChange={e => setName(e.target.value)}
            placeholder="github"
            invalid={name.length > 0 && !nameValid}
            autoComplete="off"
            data-testid="mcp-form-name"
          />
          <p className="text-[11px] text-content-muted">{t('mcp.form.nameHint')}</p>
        </div>

        {/* Transport */}
        <div className="space-y-1">
          <Label htmlFor={`${titleId}-transport`}>{t('mcp.form.transport')}</Label>
          <NativeSelect
            id={`${titleId}-transport`}
            inputSize="sm"
            value={transport}
            onChange={e => setTransport(e.target.value as Transport)}
            data-testid="mcp-form-transport">
            <option value="stdio">{t('mcp.form.transportStdio')}</option>
            <option value="http">{t('mcp.form.transportHttp')}</option>
          </NativeSelect>
        </div>

        {transport === 'stdio' ? (
          <>
            <div className="space-y-1">
              <Label htmlFor={`${titleId}-command`}>{t('mcp.form.command')}</Label>
              <TextField
                id={`${titleId}-command`}
                mono
                inputSize="sm"
                value={command}
                onChange={e => setCommand(e.target.value)}
                placeholder="npx"
                autoComplete="off"
                data-testid="mcp-form-command"
              />
            </div>
            <div className="space-y-1">
              <Label htmlFor={`${titleId}-args`}>{t('mcp.form.args')}</Label>
              <TextField
                id={`${titleId}-args`}
                mono
                inputSize="sm"
                value={args}
                onChange={e => setArgs(e.target.value)}
                placeholder="-y @modelcontextprotocol/server-github"
                autoComplete="off"
                data-testid="mcp-form-args"
              />
              <p className="text-[11px] text-content-muted">{t('mcp.form.argsHint')}</p>
            </div>
            <fieldset className="space-y-2">
              <legend className="text-sm font-medium text-content">{t('mcp.form.env')}</legend>
              <p className="text-[11px] text-content-muted">{t('mcp.form.envHint')}</p>
              {env.map((pair, index) => (
                <div key={index} className="flex items-center gap-2">
                  <TextField
                    mono
                    inputSize="sm"
                    aria-label={t('mcp.form.envKey')}
                    value={pair.key}
                    readOnly={pair.stored}
                    onChange={e => updateEnv(index, { key: e.target.value })}
                    placeholder="API_KEY"
                    autoComplete="off"
                    className="flex-1"
                  />
                  <TextField
                    inputSize="sm"
                    type="password"
                    aria-label={t('mcp.form.envValue')}
                    value={pair.value}
                    onChange={e => updateEnv(index, { value: e.target.value })}
                    placeholder={pair.stored ? t('mcp.form.keepStored') : t('mcp.form.value')}
                    autoComplete="new-password"
                    data-1p-ignore
                    className="flex-1"
                  />
                  <Button
                    variant="tertiary"
                    size="sm"
                    iconOnly
                    aria-label={t('mcp.form.removeRow')}
                    onClick={() => setEnv(prev => prev.filter((_, i) => i !== index))}>
                    <X className="size-4" aria-hidden="true" />
                  </Button>
                </div>
              ))}
              <Button
                variant="tertiary"
                size="xs"
                leadingIcon={<Plus className="size-3.5" aria-hidden="true" />}
                onClick={() => setEnv(prev => [...prev, { key: '', value: '', stored: false }])}>
                {t('mcp.form.addEnv')}
              </Button>
            </fieldset>
          </>
        ) : (
          <>
            <div className="space-y-1">
              <Label htmlFor={`${titleId}-url`}>{t('mcp.form.url')}</Label>
              <TextField
                id={`${titleId}-url`}
                mono
                inputSize="sm"
                type="url"
                value={url}
                onChange={e => setUrl(e.target.value)}
                placeholder="https://mcp.example.com/mcp"
                autoComplete="off"
                data-testid="mcp-form-url"
              />
            </div>
            <div className="space-y-1">
              <Label htmlFor={`${titleId}-auth`}>{t('mcp.form.auth')}</Label>
              <NativeSelect
                id={`${titleId}-auth`}
                inputSize="sm"
                value={authMode}
                onChange={e => setAuthMode(e.target.value as AuthMode)}
                data-testid="mcp-form-auth">
                <option value="none">{t('mcp.form.authNone')}</option>
                <option value="bearer">{t('mcp.form.authBearer')}</option>
                <option value="header">{t('mcp.form.authHeader')}</option>
                <option value="oauth">{t('mcp.form.authOauth')}</option>
              </NativeSelect>
            </div>
            {authMode === 'bearer' && (
              <div className="space-y-1">
                <Label htmlFor={`${titleId}-token`}>{t('mcp.form.token')}</Label>
                <TextField
                  id={`${titleId}-token`}
                  inputSize="sm"
                  type="password"
                  value={token}
                  onChange={e => setToken(e.target.value)}
                  placeholder={
                    storedKeys.some(k => k.toLowerCase() === 'authorization')
                      ? t('mcp.form.keepStored')
                      : 'sk-…'
                  }
                  autoComplete="new-password"
                  data-1p-ignore
                  data-testid="mcp-form-token"
                />
                <p className="text-[11px] text-content-muted">{t('mcp.form.tokenHint')}</p>
              </div>
            )}
            {authMode === 'header' && (
              <div className="flex items-center gap-2">
                <div className="flex-1 space-y-1">
                  <Label htmlFor={`${titleId}-hname`}>{t('mcp.form.headerName')}</Label>
                  <TextField
                    id={`${titleId}-hname`}
                    mono
                    inputSize="sm"
                    value={headerName}
                    onChange={e => setHeaderName(e.target.value)}
                    placeholder="X-API-Key"
                    autoComplete="off"
                  />
                </div>
                <div className="flex-1 space-y-1">
                  <Label htmlFor={`${titleId}-hvalue`}>{t('mcp.form.headerValue')}</Label>
                  <TextField
                    id={`${titleId}-hvalue`}
                    inputSize="sm"
                    type="password"
                    value={headerValue}
                    onChange={e => setHeaderValue(e.target.value)}
                    placeholder={
                      storedKeys.includes(headerName.trim())
                        ? t('mcp.form.keepStored')
                        : t('mcp.form.value')
                    }
                    autoComplete="new-password"
                    data-1p-ignore
                  />
                </div>
              </div>
            )}
            {authMode === 'oauth' && (
              <p className="text-xs text-content-muted">{t('mcp.form.oauthHint')}</p>
            )}
          </>
        )}

        {error && (
          <Alert variant="destructive" density="compact" data-testid="mcp-form-error">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
      </form>
    </ModalShell>
  );
};

export default McpServerForm;
