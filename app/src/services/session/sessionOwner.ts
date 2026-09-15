/**
 * The frontend's one door to "whoever owns the backend session".
 *
 * The core only *holds* a credential (`auth.set_credential`). Obtaining and
 * validating one — login-token exchange, `GET /auth/me`, the current-user
 * cache — is the host's job:
 *
 * - On the desktop, the Tauri shell owns it (`crates/openhuman-session`
 *   behind the `auth_*` commands). This module invokes those commands.
 * - In the browser build, and in cloud mode (the renderer talks to a remote
 *   core the shell knows nothing about), there is no shell owner. A thin
 *   TypeScript equivalent does the same three steps against the backend and
 *   then hands the credential to the core over RPC. It is deliberately
 *   minimal: no deferred validation, no background revalidation.
 *
 * Error messages from either path start with a stable `PREFIX:` (`REJECTED`,
 * `EXPIRED`, `TRANSIENT`, `CONSUME_FAILED`, `CORE`, …) so callers can classify
 * without parsing prose.
 */
import { getStoredCoreMode } from '../../utils/configPersistence';
import { isLocalSessionToken } from '../../utils/localSession';
import { safeInvoke as invoke, isTauri } from '../../utils/tauriCommands/common';
import { getBackendUrl } from '../backendUrl';
import { getClientVersionHeaders } from '../clientVersionHeaders';
import { callCoreRpc } from '../coreRpcClient';

export interface SessionCurrentUser {
  user: object | null;
  stale: boolean;
  staleSeconds: number | null;
}

export interface SessionOwnerState {
  isAuthenticated: boolean;
  credential?: string | null;
  userId: string | null;
  user: object | null;
  profileId: string | null;
  expiresAt?: string | null;
  currentUser: object | null;
  currentUserStale: boolean;
  currentUserStaleSeconds: number | null;
}

const AUTH_ME_TIMEOUT_MS = 12_000;

/**
 * Whether the Tauri shell is the session owner for the core the renderer is
 * talking to. Cloud mode points the renderer at a remote core through its own
 * stored RPC endpoint, which the shell does not know about, so the browser
 * owner drives that core over the same RPC path the renderer already uses.
 */
export const useShellSessionOwner = (): boolean => isTauri() && getStoredCoreMode() !== 'cloud';

const withTimeout = async <T>(promise: Promise<T>, ms: number, what: string): Promise<T> => {
  let timer: number | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = window.setTimeout(() => reject(new Error(`TRANSIENT: ${what} timed out`)), ms);
  });
  try {
    return await Promise.race([promise, timeout]);
  } finally {
    window.clearTimeout(timer);
  }
};

// ── browser / cloud owner ─────────────────────────────────────

const backendFetch = async (path: string, init: RequestInit): Promise<Response> => {
  const backendUrl = await getBackendUrl();
  const versionHeaders = await getClientVersionHeaders();
  return withTimeout(
    fetch(`${backendUrl}${path}`, {
      ...init,
      headers: { 'Content-Type': 'application/json', ...versionHeaders, ...(init.headers ?? {}) },
    }),
    AUTH_ME_TIMEOUT_MS,
    path
  );
};

const unwrapEnvelope = (value: unknown): unknown => {
  if (value && typeof value === 'object') {
    const obj = value as Record<string, unknown>;
    if (obj.success === false) {
      const message = (obj.message ?? obj.error ?? 'request failed') as string;
      throw new Error(`REJECTED: ${message}`);
    }
    if (obj.data !== undefined && obj.data !== null) return unwrapEnvelope(obj.data);
    if (obj.user !== undefined && obj.user !== null) return obj.user;
  }
  return value;
};

/**
 * `POST /auth/login-token/consume` → JWT. Works for OAuth and Telegram login
 * tokens alike.
 */
const browserConsumeLoginToken = async (loginToken: string): Promise<string> => {
  const response = await backendFetch('/auth/login-token/consume', {
    method: 'POST',
    body: JSON.stringify({ token: loginToken }),
  });
  if (!response.ok) {
    throw new Error(`CONSUME_FAILED: login token consume returned ${response.status}`);
  }
  const data = unwrapEnvelope(await response.json().catch(() => ({}))) as {
    jwt?: string;
    jwtToken?: string;
  };
  const jwt = (data.jwt ?? data.jwtToken ?? '').trim();
  if (!jwt) throw new Error('CONSUME_FAILED: login token consume response missing jwt');
  return jwt;
};

/** `GET /auth/me` with the session JWT. */
const browserFetchMe = async (token: string): Promise<object> => {
  const response = await backendFetch('/auth/me', {
    method: 'GET',
    headers: { Authorization: `Bearer ${token}` },
  });
  if (response.status === 401 || response.status === 403) {
    throw new Error(`REJECTED: backend rejected the session token (${response.status})`);
  }
  if (!response.ok) {
    throw new Error(`TRANSIENT: GET /auth/me returned ${response.status}`);
  }
  return unwrapEnvelope(await response.json()) as object;
};

const userIdFromPayload = (user: unknown): string | undefined => {
  if (!user || typeof user !== 'object') return undefined;
  const obj = user as Record<string, unknown>;
  for (const key of ['id', '_id', 'userId']) {
    const value = obj[key];
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return undefined;
};

const coreSetCredential = async (params: {
  token: string;
  kind?: 'session' | 'local' | 'api-key';
  userId?: string;
  user?: object;
}): Promise<void> => {
  try {
    await callCoreRpc({ method: 'openhuman.auth_set_credential', params });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(message.startsWith('CORE:') ? message : `CORE: ${message}`);
  }
};

const browserStoreSessionToken = async (token: string, user?: object): Promise<void> => {
  if (isLocalSessionToken(token)) {
    await coreSetCredential({ token, kind: 'local', user });
    return;
  }
  const me = await browserFetchMe(token);
  await coreSetCredential({ token, kind: 'session', userId: userIdFromPayload(me), user: me });
};

const browserCurrentUser = async (): Promise<SessionCurrentUser> => {
  const state = await callCoreRpc<{
    result: { isAuthenticated: boolean; credential?: string | null; user: object | null };
  }>({ method: 'openhuman.auth_get_state' });
  const core = state.result;
  if (!core.isAuthenticated) return { user: null, stale: false, staleSeconds: null };
  if (core.credential !== 'session') return { user: core.user, stale: false, staleSeconds: null };
  const tokenResponse = await callCoreRpc<{ result: { token: string | null } }>({
    method: 'openhuman.auth_get_session_token',
  });
  const token = tokenResponse.result.token;
  if (!token) return { user: core.user, stale: false, staleSeconds: null };
  try {
    return { user: await browserFetchMe(token), stale: false, staleSeconds: 0 };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (message.startsWith('REJECTED:')) {
      await callCoreRpc({
        method: 'openhuman.auth_clear_credential',
        params: { kind: 'session' },
      }).catch(() => undefined);
      throw error;
    }
    return { user: core.user, stale: true, staleSeconds: null };
  }
};

// ── public surface ────────────────────────────────────────────

/** Exchange a one-time login token for a session and install it. */
export const loginWithToken = async (loginToken: string): Promise<void> => {
  if (useShellSessionOwner()) {
    await invoke('auth_login_with_token', { token: loginToken });
    return;
  }
  const jwt = await browserConsumeLoginToken(loginToken);
  await browserStoreSessionToken(jwt);
};

/**
 * Install a session JWT (validated against the backend first) or the offline
 * local token (stored as-is with `user`).
 */
export const storeSessionToken = async (token: string, user?: object): Promise<void> => {
  if (useShellSessionOwner()) {
    await invoke('auth_store_session', { token, user: user ?? null });
    return;
  }
  await browserStoreSessionToken(token, user);
};

/** Sign out: clear the session credential in the core. */
export const logoutSession = async (): Promise<void> => {
  if (useShellSessionOwner()) {
    await invoke('auth_logout');
    return;
  }
  await callCoreRpc({ method: 'openhuman.auth_clear_credential', params: { kind: 'session' } });
};

/**
 * The current user from `/auth/me` — cached by the shell owner unless
 * `force`. Throws `REJECTED:` (and clears the session) when the backend no
 * longer accepts the credential; serves the stored user flagged `stale` when
 * the backend is unreachable.
 */
export const fetchCurrentUser = async (force = false): Promise<SessionCurrentUser> => {
  if (useShellSessionOwner()) {
    const cached = await invoke<{
      user: object | null;
      stale: boolean;
      staleSeconds: number | null;
    }>('auth_current_user', { force });
    return { user: cached.user, stale: cached.stale, staleSeconds: cached.staleSeconds };
  }
  return browserCurrentUser();
};

/** The shell owner's full view (desktop only); `null` elsewhere. */
export const fetchSessionOwnerState = async (): Promise<SessionOwnerState | null> => {
  if (!useShellSessionOwner()) return null;
  return invoke<SessionOwnerState>('auth_state');
};

/** Classify an owner error message by its stable prefix. */
export const sessionErrorKind = (
  message: string
): 'rejected' | 'expired' | 'transient' | 'consume_failed' | 'core' | 'user_id' | 'other' => {
  if (message.startsWith('REJECTED:')) return 'rejected';
  if (message.startsWith('EXPIRED:')) return 'expired';
  if (message.startsWith('TRANSIENT:')) return 'transient';
  if (message.startsWith('CONSUME_FAILED:')) return 'consume_failed';
  if (message.startsWith('CORE:') || message.startsWith('BACKEND:')) return 'core';
  if (message.startsWith('USER_ID_UNAVAILABLE:')) return 'user_id';
  return 'other';
};
