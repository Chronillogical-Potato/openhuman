import { fireEvent, screen, waitFor } from '@testing-library/react';
import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { clearBackendUrlCache } from '../../services/backendUrl';
import { clearCoreRpcTokenCache, clearCoreRpcUrlCache } from '../../services/coreRpcClient';
import {
  endAwaitingAuthCallback,
  getDeepLinkAuthState,
  useDeepLinkAuthState,
} from '../../store/deepLinkAuthState';
import { renderWithProviders } from '../../test/test-utils';
import {
  clearStoredCoreMode,
  clearStoredCoreToken,
  storeRpcUrl,
} from '../../utils/configPersistence';
import { PRIVACY_POLICY_URL, TERMS_OF_USE_URL } from '../../utils/links';
import { openUrl } from '../../utils/openUrl';
import Welcome from '../Welcome';

const mockStoreSessionToken = vi.fn().mockResolvedValue(undefined);

vi.mock('../../providers/CoreStateProvider', () => ({
  useCoreState: () => ({ storeSessionToken: mockStoreSessionToken }),
}));

const oauthButtonSpy = vi.fn();
const oauthOverrideSpy = vi.fn();

vi.mock('../../components/oauth/OAuthProviderButton', () => ({
  default: ({
    provider,
    onClickOverride,
  }: {
    provider: { id: string };
    onClickOverride?: () => void;
  }) => (
    <button
      type="button"
      onClick={() => {
        oauthButtonSpy(provider.id);
        if (onClickOverride) {
          oauthOverrideSpy(provider.id);
          onClickOverride();
        }
      }}>
      {provider.id}
    </button>
  ),
}));

vi.mock('../../components/oauth/providerConfigs', () => ({
  oauthProviderConfigs: [
    { id: 'google', showOnWelcome: true },
    { id: 'github', showOnWelcome: true },
    { id: 'twitter', showOnWelcome: true },
    { id: 'discord', showOnWelcome: false },
  ],
}));

vi.mock('../../store/deepLinkAuthState', () => ({
  useDeepLinkAuthState: vi.fn(),
  getDeepLinkAuthState: vi.fn(),
  endAwaitingAuthCallback: vi.fn(),
}));

const mockNavigate = vi.fn();
vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof import('react-router-dom')>('react-router-dom');
  return { ...actual, useNavigate: () => mockNavigate };
});

const { mockClearAllAppData } = vi.hoisted(() => ({
  mockClearAllAppData: vi.fn().mockResolvedValue(undefined),
}));
vi.mock('../../utils/clearAllAppData', () => ({
  clearAllAppData: (...args: unknown[]) => mockClearAllAppData(...args),
}));

vi.mock('../../utils/openUrl', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));

vi.mock('../../services/coreRpcClient', () => ({
  clearCoreRpcUrlCache: vi.fn(),
  clearCoreRpcTokenCache: vi.fn(),
  testCoreRpcConnection: vi.fn(),
}));

vi.mock('../../services/backendUrl', () => ({
  clearBackendUrlCache: vi.fn(),
  getBackendUrl: vi.fn().mockResolvedValue('http://localhost:5005'),
}));

vi.mock('../../utils/configPersistence', () => ({
  getStoredRpcUrl: vi.fn(() => 'http://127.0.0.1:7788/rpc'),
  peekStoredRpcUrl: vi.fn(() => null),
  storeRpcUrl: vi.fn(),
  clearStoredRpcUrl: vi.fn(),
  getStoredCoreToken: vi.fn(() => null),
  storeCoreToken: vi.fn(),
  clearStoredCoreToken: vi.fn(),
  getStoredCoreMode: vi.fn(() => null),
  storeCoreMode: vi.fn(),
  clearStoredCoreMode: vi.fn(),
  getDefaultRpcUrl: vi.fn(() => 'http://127.0.0.1:7788/rpc'),
  isValidRpcUrl: vi.fn((url: string) => {
    if (!url || url.trim().length === 0) return false;
    try {
      const parsed = new URL(url);
      return parsed.protocol === 'http:' || parsed.protocol === 'https:';
    } catch {
      return false;
    }
  }),
  normalizeRpcUrl: vi.fn((url: string) => url.trim().replace(/\/+$/, '')),
}));

/**
 * The provider buttons are rendered unconditionally now, so there is nothing
 * to reveal. Kept as a no-op so the call sites below still read as "the
 * providers are available at this point" rather than being silently deleted.
 */
function revealProviders() {
  expect(screen.getByTestId('welcome-cta-tinyhumans')).toBeInTheDocument();
}

describe('Welcome — two-card layout', () => {
  beforeEach(() => {
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: false,
      awaitingCallback: false,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    });
  });

  it('renders both cards with their CTAs and the server link', () => {
    renderWithProviders(<Welcome />);

    expect(screen.getByTestId('welcome-card-tinyhumans')).toBeInTheDocument();
    expect(screen.getByTestId('welcome-card-self')).toBeInTheDocument();
    // The TinyHumans card's action IS the provider row — there is no
    // intermediate button to click through.
    expect(screen.getByTestId('welcome-cta-tinyhumans')).toBeInTheDocument();
    expect(screen.getByTestId('welcome-cta-self')).toHaveTextContent('Set it up myself');
    expect(screen.getByTestId('welcome-server-link')).toHaveTextContent('Connect to it.');
  });

  it('shows the sign-in providers immediately, with no reveal step', () => {
    // A "Continue with TinyHumans" button used to gate these. It revealed the
    // buttons and did nothing else, so it was a tap that bought nothing.
    renderWithProviders(<Welcome />);

    expect(screen.getByRole('button', { name: 'google' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'github' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'twitter' })).toBeInTheDocument();
    // `showOnWelcome: false` still keeps discord out.
    expect(screen.queryByRole('button', { name: 'discord' })).not.toBeInTheDocument();
  });
});

describe('Welcome auth entrypoint', () => {
  beforeEach(() => {
    oauthButtonSpy.mockReset();
    oauthOverrideSpy.mockReset();
    vi.mocked(openUrl).mockClear();
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: false,
      awaitingCallback: false,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    });
  });

  it('renders only the OAuth buttons when auth is idle', () => {
    renderWithProviders(<Welcome />);
    revealProviders();

    expect(screen.queryByLabelText('Email address')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Continue with email' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'google' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'github' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'twitter' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'discord' })).not.toBeInTheDocument();
  });

  it('renders legal links with valid targets and opens them externally', () => {
    renderWithProviders(<Welcome />);

    const termsLink = screen.getByRole('link', { name: 'Terms' });
    const privacyLink = screen.getByRole('link', { name: 'Privacy Policy' });

    expect(termsLink).toHaveAttribute('href', TERMS_OF_USE_URL);
    expect(privacyLink).toHaveAttribute('href', PRIVACY_POLICY_URL);

    fireEvent.click(termsLink);
    fireEvent.click(privacyLink);

    expect(openUrl).toHaveBeenNthCalledWith(1, TERMS_OF_USE_URL);
    expect(openUrl).toHaveBeenNthCalledWith(2, PRIVACY_POLICY_URL);
  });

  it('delegates OAuth clicks to OAuthProviderButton without an override', () => {
    renderWithProviders(<Welcome />);
    revealProviders();

    fireEvent.click(screen.getByRole('button', { name: 'google' }));
    fireEvent.click(screen.getByRole('button', { name: 'github' }));
    fireEvent.click(screen.getByRole('button', { name: 'twitter' }));

    expect(oauthButtonSpy).toHaveBeenNthCalledWith(1, 'google');
    expect(oauthButtonSpy).toHaveBeenNthCalledWith(2, 'github');
    expect(oauthButtonSpy).toHaveBeenNthCalledWith(3, 'twitter');
    expect(oauthOverrideSpy).not.toHaveBeenCalled();
  });

  it('shows the deep-link processing state when auth is already in progress', () => {
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: true,
      awaitingCallback: false,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    });

    renderWithProviders(<Welcome />);

    expect(screen.getByTestId('welcome-handoff')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Finishing sign-in in your browser');
    // No sign-in buttons on the hand-off: a provider has already been picked
    // and the browser is open, so repeating the row asks the same question
    // twice. The 90s timeout's failure state is where the way out lives.
    expect(screen.queryByTestId('welcome-handoff-reopen')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'google' })).not.toBeInTheDocument();
    // The two cards are replaced by the hand-off screen while auth is in flight.
    expect(screen.queryByTestId('welcome-card-tinyhumans')).not.toBeInTheDocument();
  });

  it('shows the hand-off panel while awaiting the browser callback, even though no auth step is running', () => {
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: false,
      awaitingCallback: true,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    });

    renderWithProviders(<Welcome />);

    expect(screen.getByTestId('welcome-handoff')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Finishing sign-in in your browser');
    expect(screen.queryByTestId('welcome-card-tinyhumans')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'google' })).not.toBeInTheDocument();
  });

  it('ends the wait when the window regains focus with no callback (cancelled browser flow)', () => {
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: false,
      awaitingCallback: true,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    });
    vi.mocked(getDeepLinkAuthState).mockReturnValue({
      isProcessing: false,
      awaitingCallback: true,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    });
    vi.mocked(endAwaitingAuthCallback).mockClear();

    vi.useFakeTimers();
    renderWithProviders(<Welcome />);
    act(() => {
      window.dispatchEvent(new FocusEvent('focus'));
    });

    // The grace window has to elapse first: a successful callback arrives
    // after the focus event, so ending the wait on focus alone would cut a
    // good sign-in short.
    expect(endAwaitingAuthCallback).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(1_500);
    });
    expect(endAwaitingAuthCallback).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });

  it('keeps the hand-off when the callback lands during the grace window', () => {
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: false,
      awaitingCallback: true,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    });
    // The real ordering: focus fires while the store still says no auth step
    // is running, and the deep link lands a moment later. Reading the store
    // the instant focus arrives therefore says "cancelled" during a perfectly
    // good sign-in, which is why the decision is deferred.
    let storeProcessing = false;
    vi.mocked(getDeepLinkAuthState).mockImplementation(() => ({
      isProcessing: storeProcessing,
      awaitingCallback: true,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    }));
    vi.mocked(endAwaitingAuthCallback).mockClear();

    vi.useFakeTimers();
    renderWithProviders(<Welcome />);
    act(() => {
      window.dispatchEvent(new FocusEvent('focus'));
    });
    // The callback arrives inside the grace window.
    act(() => {
      vi.advanceTimersByTime(500);
    });
    storeProcessing = true;
    act(() => {
      vi.advanceTimersByTime(1_000);
    });

    expect(endAwaitingAuthCallback).not.toHaveBeenCalled();
    expect(screen.getByTestId('welcome-handoff')).toBeInTheDocument();
    vi.useRealTimers();
  });

  it('keeps waiting on focus when a callback is already being redeemed', () => {
    const redeeming = {
      isProcessing: true,
      awaitingCallback: true,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    };
    vi.mocked(useDeepLinkAuthState).mockReturnValue(redeeming);
    vi.mocked(getDeepLinkAuthState).mockReturnValue(redeeming);
    vi.mocked(endAwaitingAuthCallback).mockClear();

    vi.useFakeTimers();
    renderWithProviders(<Welcome />);
    act(() => {
      window.dispatchEvent(new FocusEvent('focus'));
    });
    act(() => {
      vi.advanceTimersByTime(1_500);
    });

    expect(endAwaitingAuthCallback).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it('offers retry and a self-hosted fallback when the browser hand-off fails', async () => {
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: false,
      awaitingCallback: false,
      errorMessage: 'OAuth failed',
      errorMessageKey: null,
      requiresAppDataReset: false,
    });

    renderWithProviders(<Welcome />);

    expect(screen.getByRole('alert')).toHaveTextContent("Sign-in didn't come back");
    // Retrying means clicking a provider again, and the card keeps them on
    // screen — the alert deliberately does not repeat them, which would put
    // two Google buttons in the document.
    expect(screen.getAllByRole('button', { name: 'google' })).toHaveLength(1);

    // The retry itself: clicking the provider starts a sign-in again.
    oauthButtonSpy.mockClear();
    fireEvent.click(screen.getByRole('button', { name: 'google' }));
    expect(oauthButtonSpy).toHaveBeenCalledTimes(1);
    expect(oauthButtonSpy).toHaveBeenCalledWith('google');

    fireEvent.click(screen.getByTestId('welcome-handoff-fallback-self'));
    await waitFor(() => expect(mockStoreSessionToken).toHaveBeenCalledTimes(1));
    expect(mockNavigate).toHaveBeenCalledWith('/onboarding/custom/inference', { replace: true });
  });

  it('renders deep-link auth errors', () => {
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: false,
      awaitingCallback: false,
      errorMessage: 'OAuth failed',
      errorMessageKey: null,
      requiresAppDataReset: false,
    });

    renderWithProviders(<Welcome />);

    expect(screen.getByRole('alert')).toHaveTextContent('OAuth failed');
    expect(
      screen.queryByRole('button', { name: /Clear app data & restart/ })
    ).not.toBeInTheDocument();
  });
});

describe('Welcome — decryption-failure recovery action', () => {
  beforeEach(() => {
    mockClearAllAppData.mockReset().mockResolvedValue(undefined);
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: false,
      awaitingCallback: false,
      errorMessage: "Sign-in failed because OpenHuman couldn't decrypt locally stored data.",
      errorMessageKey: null,
      requiresAppDataReset: true,
    });
  });

  it('renders the "Clear app data & restart" button when reset is required', () => {
    renderWithProviders(<Welcome />);

    expect(screen.getByRole('button', { name: /Clear app data & restart/ })).toBeInTheDocument();
    expect(screen.getByText(/cloud account is unaffected/i)).toBeInTheDocument();
  });

  it('invokes clearAllAppData on click', async () => {
    renderWithProviders(<Welcome />);

    fireEvent.click(screen.getByRole('button', { name: /Clear app data & restart/ }));

    await waitFor(() => expect(mockClearAllAppData).toHaveBeenCalledTimes(1));
    // Pre-login path: no clearSession callback is passed.
    expect(mockClearAllAppData).toHaveBeenCalledWith();
  });

  it('surfaces an inline error if clearAllAppData rejects', async () => {
    mockClearAllAppData.mockRejectedValueOnce(new Error('reset blew up'));

    renderWithProviders(<Welcome />);
    fireEvent.click(screen.getByRole('button', { name: /Clear app data & restart/ }));

    await waitFor(() => {
      expect(screen.getByText(/reset blew up/)).toBeInTheDocument();
    });
    // Button re-enabled so the user can retry.
    expect(screen.getByRole('button', { name: /Clear app data & restart/ })).not.toBeDisabled();
  });

  it('falls back to the generic message when the error has no message', async () => {
    mockClearAllAppData.mockRejectedValueOnce(new Error(''));

    renderWithProviders(<Welcome />);
    fireEvent.click(screen.getByRole('button', { name: /Clear app data & restart/ }));

    await waitFor(() => {
      expect(screen.getByText(/Could not clear app data/)).toBeInTheDocument();
    });
  });
});

describe('Welcome — server link (Connect to it.)', () => {
  beforeEach(() => {
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: false,
      awaitingCallback: false,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    });
    vi.mocked(clearCoreRpcUrlCache).mockReset();
    vi.mocked(clearCoreRpcTokenCache).mockReset();
    vi.mocked(clearBackendUrlCache).mockReset();
    vi.mocked(storeRpcUrl).mockReset();
    vi.mocked(clearStoredCoreToken).mockReset();
    vi.mocked(clearStoredCoreMode).mockReset();
  });

  it('renders the "Connect to it." link below the cards', () => {
    renderWithProviders(<Welcome />);

    expect(screen.getByTestId('welcome-server-link')).toBeInTheDocument();
  });

  it('does not render the legacy "Configure RPC URL (Advanced)" toggle', () => {
    renderWithProviders(<Welcome />);

    expect(
      screen.queryByRole('button', { name: 'Configure RPC URL (Advanced)' })
    ).not.toBeInTheDocument();
  });

  it('clicking "Connect to it." clears persisted core-mode state and resets caches', () => {
    const { store } = renderWithProviders(<Welcome />, {
      preloadedState: { coreMode: { mode: { kind: 'cloud', url: 'http://x', token: 't' } } },
    });

    fireEvent.click(screen.getByTestId('welcome-server-link'));

    expect(storeRpcUrl).toHaveBeenCalledWith('');
    expect(clearStoredCoreToken).toHaveBeenCalledTimes(1);
    expect(clearStoredCoreMode).toHaveBeenCalledTimes(1);
    expect(clearCoreRpcUrlCache).toHaveBeenCalledTimes(1);
    expect(clearCoreRpcTokenCache).toHaveBeenCalledTimes(1);
    expect(clearBackendUrlCache).toHaveBeenCalledTimes(1);
    // Redux coreMode slice is reset to `unset` so BootCheckGate returns to picker.
    expect((store.getState() as { coreMode: { mode: { kind: string } } }).coreMode.mode.kind).toBe(
      'unset'
    );
  });
});

describe('Welcome — OAuth buttons presence', () => {
  beforeEach(() => {
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: false,
      awaitingCallback: false,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    });
  });

  it('renders all providers with showOnWelcome=true', () => {
    renderWithProviders(<Welcome />);
    revealProviders();

    expect(screen.getByRole('button', { name: 'google' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'github' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'twitter' })).toBeInTheDocument();
  });

  it('does not render providers with showOnWelcome=false', () => {
    renderWithProviders(<Welcome />);
    revealProviders();

    expect(screen.queryByRole('button', { name: 'discord' })).not.toBeInTheDocument();
  });

  it('swaps the cards for the hand-off panel while auth is processing', () => {
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: true,
      awaitingCallback: false,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    });
    renderWithProviders(<Welcome />);

    // The two choice cards give way to the hand-off panel...
    expect(screen.queryByTestId('welcome-card-tinyhumans')).not.toBeInTheDocument();
    expect(screen.getByTestId('welcome-handoff')).toBeInTheDocument();

    // ...and carries no sign-in buttons of its own. A provider has already
    // been chosen and the browser is open; the row would just repeat the
    // question. Recovery lives in the 90s failure state, not here.
    expect(screen.queryByTestId('welcome-handoff-reopen')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'google' })).not.toBeInTheDocument();
  });
});

describe('Welcome — local login', () => {
  beforeEach(() => {
    mockStoreSessionToken.mockReset().mockResolvedValue(undefined);
    mockNavigate.mockReset();
    vi.mocked(useDeepLinkAuthState).mockReturnValue({
      isProcessing: false,
      awaitingCallback: false,
      errorMessage: null,
      errorMessageKey: null,
      requiresAppDataReset: false,
    });
  });

  it('renders the "Set it up myself" button regardless of runtime mode', () => {
    renderWithProviders(<Welcome />);

    expect(screen.getByTestId('welcome-cta-self')).toBeInTheDocument();
  });

  it('renders the "Set it up myself" button in cloud mode too', () => {
    renderWithProviders(<Welcome />, {
      preloadedState: { coreMode: { mode: { kind: 'cloud', url: 'http://x', token: 't' } } },
    });

    expect(screen.getByTestId('welcome-cta-self')).toBeInTheDocument();
  });

  it('"Set it up myself" starts a local session and goes to the custom wizard', async () => {
    renderWithProviders(<Welcome />);

    const localBtn = screen.getByTestId('welcome-cta-self');
    fireEvent.click(localBtn);

    await waitFor(() => {
      expect(mockStoreSessionToken).toHaveBeenCalledTimes(1);
    });
    const [tokenArg, userArg] = mockStoreSessionToken.mock.calls[0];
    // A local session token is a 3-part JWT whose signature segment is "local".
    expect(tokenArg.split('.')).toHaveLength(3);
    expect(tokenArg.split('.')[2]).toBe('local');
    expect(userArg).toEqual(expect.objectContaining({ id: 'local' }));
    expect(mockNavigate).toHaveBeenCalledWith('/onboarding/custom/inference', { replace: true });
  });

  it('shows error when storeSessionToken rejects', async () => {
    mockStoreSessionToken.mockRejectedValueOnce(new Error('token save failed'));

    renderWithProviders(<Welcome />);

    const localBtn = screen.getByTestId('welcome-cta-self');
    fireEvent.click(localBtn);

    await waitFor(() => {
      expect(screen.getByText(/token save failed/)).toBeInTheDocument();
    });
  });

  // The reported sign-in failure: the core (typically a container runtime whose
  // workspace volume carries a config.toml owned by another uid) cannot read
  // its own config, and the raw anyhow chain — absolute path and errno included
  // — was painted straight into this screen with nothing the user could act on.
  it('replaces an unreadable-core-config error with actionable, path-free copy', async () => {
    mockStoreSessionToken.mockRejectedValueOnce(
      new Error(
        'Failed to read config file: /home/openhuman/.openhuman/config.toml ' +
          '[config owner mismatch] (file uid=0 gid=0 mode=0600; process euid=10001 egid=10001): ' +
          'Permission denied (os error 13)'
      )
    );

    renderWithProviders(<Welcome />);
    fireEvent.click(screen.getByTestId('welcome-cta-self'));

    await waitFor(() => {
      expect(screen.getByText(/could not read its configuration file/i)).toBeInTheDocument();
    });
    // The raw chain must not leak: the path belongs to the runtime host, and
    // neither it nor the errno is actionable for the person signing in.
    expect(screen.queryByText(/\/home\/openhuman/)).not.toBeInTheDocument();
    expect(screen.queryByText(/os error 13/)).not.toBeInTheDocument();
    // Resolved through i18n, so a bare key must never reach the DOM.
    expect(screen.queryByText('welcome.coreConfigUnreadable')).not.toBeInTheDocument();
  });
});
