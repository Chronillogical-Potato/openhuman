import { useSyncExternalStore } from 'react';

interface DeepLinkAuthState {
  /**
   * An auth STEP is executing. True twice per sign-in and briefly each time:
   * once while the login URL is being prepared and the browser opened, and
   * again while an arriving deep link is redeemed. It is NOT "waiting for the
   * user to finish in the browser" — `awaitingCallback` is.
   */
  isProcessing: boolean;
  /**
   * The user has been handed to the system browser and the callback has not
   * come back yet. Spans the whole time the app is idle waiting on another
   * window, which is exactly the window a hand-off screen must cover.
   */
  awaitingCallback: boolean;
  errorMessage: string | null;
  // i18n key to render INSTEAD of `errorMessage`, for failures whose copy is
  // translated. This module is reached from non-React code (the deep-link
  // listener), so it cannot call `useT()` itself; carrying the key lets the
  // rendering component resolve it in the user's locale. Null keeps the
  // existing behaviour of rendering `errorMessage` verbatim.
  errorMessageKey: string | null;
  // Set when sign-in fails because the local core could not decrypt persisted
  // secrets — typically the encryption key on disk no longer matches the
  // ciphertext (key rotated, profile copied between machines, tampered/corrupt
  // storage). The only safe recovery is wiping local app data so the next
  // login starts from a clean slate.
  requiresAppDataReset: boolean;
}

const initialState: DeepLinkAuthState = {
  isProcessing: false,
  awaitingCallback: false,
  errorMessage: null,
  errorMessageKey: null,
  requiresAppDataReset: false,
};

let deepLinkAuthState: DeepLinkAuthState = initialState;
const listeners = new Set<() => void>();

const emitChange = (): void => {
  for (const listener of listeners) {
    listener();
  }
};

const setDeepLinkAuthState = (next: DeepLinkAuthState): void => {
  deepLinkAuthState = next;
  emitChange();
};

export const getDeepLinkAuthState = (): DeepLinkAuthState => deepLinkAuthState;

export const subscribeDeepLinkAuthState = (listener: () => void): (() => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

export const beginDeepLinkAuthProcessing = (): void => {
  clearAwaitTimer();
  setDeepLinkAuthState({
    isProcessing: true,
    // A callback that has arrived is no longer awaited.
    awaitingCallback: false,
    errorMessage: null,
    errorMessageKey: null,
    requiresAppDataReset: false,
  });
};

export const completeDeepLinkAuthProcessing = (): void => {
  setDeepLinkAuthState({
    isProcessing: false,
    awaitingCallback: deepLinkAuthState.awaitingCallback,
    errorMessage: null,
    errorMessageKey: null,
    requiresAppDataReset: false,
  });
};

export const failDeepLinkAuthProcessing = (
  message: string,
  options: { requiresAppDataReset?: boolean; messageKey?: string } = {}
): void => {
  clearAwaitTimer();
  setDeepLinkAuthState({
    isProcessing: false,
    awaitingCallback: false,
    errorMessage: message,
    errorMessageKey: options.messageKey ?? null,
    requiresAppDataReset: Boolean(options.requiresAppDataReset),
  });
};

/**
 * How long to hold the hand-off before giving up on the browser.
 *
 * Matches the loopback listener's own lifetime: past this point no callback
 * can arrive, so continuing to show a spinner is a lie.
 */
const AWAIT_CALLBACK_TIMEOUT_MS = 300_000;
let awaitTimer: ReturnType<typeof setTimeout> | null = null;

const clearAwaitTimer = (): void => {
  if (awaitTimer === null) return;
  clearTimeout(awaitTimer);
  awaitTimer = null;
};

/**
 * The browser is open and we are waiting on it. Called right after `openUrl`
 * succeeds, because `completeDeepLinkAuthProcessing()` fires immediately after
 * that and would otherwise drop the UI back to the sign-in screen while the
 * user is still in the browser.
 */
export const beginAwaitingAuthCallback = (): void => {
  clearAwaitTimer();
  // The timer lives here, not in the component that started the sign-in: the
  // hand-off screen unmounts that component, which would take the only way of
  // ever clearing this flag with it and strand the user on a spinner with no
  // control. Module scope outlives any mount.
  awaitTimer = setTimeout(() => {
    awaitTimer = null;
    endAwaitingAuthCallback();
  }, AWAIT_CALLBACK_TIMEOUT_MS);
  setDeepLinkAuthState({ ...deepLinkAuthState, awaitingCallback: true });
};

/** Stop waiting — the callback arrived, failed, or the user gave up. */
export const endAwaitingAuthCallback = (): void => {
  clearAwaitTimer();
  if (!deepLinkAuthState.awaitingCallback) return;
  setDeepLinkAuthState({ ...deepLinkAuthState, awaitingCallback: false });
};

export const useDeepLinkAuthState = (): DeepLinkAuthState =>
  useSyncExternalStore(subscribeDeepLinkAuthState, getDeepLinkAuthState, getDeepLinkAuthState);
