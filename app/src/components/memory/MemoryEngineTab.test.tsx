import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { EngineState } from '../../services/api/memoryApi';
import { renderWithProviders } from '../../test/test-utils';
import { createLocalSessionToken } from '../../utils/localSession';
import MemoryEngineTab, {
  CORTEXDB_SELF_HOST_DOCS_URL,
  isLoopbackEndpoint,
} from './MemoryEngineTab';

const hoisted = vi.hoisted(() => ({
  engineSet: vi.fn(),
  openUrl: vi.fn(),
  signedIn: true,
  token: 'header.payload.sig',
  plan: null as string | null,
  toastAdd: vi.fn(),
}));

vi.mock('../ui/Toast', () => ({ toast: { add: (...a: unknown[]) => hoisted.toastAdd(...a) } }));

vi.mock('../../services/api/memoryApi', async importOriginal => ({
  ...(await importOriginal<typeof import('../../services/api/memoryApi')>()),
  memoryEngineSet: (...a: unknown[]) => hoisted.engineSet(...a),
}));

vi.mock('../../utils/openUrl', () => ({ openUrl: (...a: unknown[]) => hoisted.openUrl(...a) }));

vi.mock('../../providers/CoreStateProvider', () => ({
  useCoreState: () => ({
    snapshot: {
      auth: { isAuthenticated: hoisted.signedIn, userId: hoisted.signedIn ? 'u1' : null },
      sessionToken: hoisted.signedIn ? hoisted.token : null,
      currentUser: hoisted.plan ? { subscription: { plan: hoisted.plan } } : null,
    },
  }),
}));

const OFF: EngineState = { engine: null, has_key: false, status: 'off', fetch_modes: [] };
const BUILTIN_ON: EngineState = {
  engine: 'tinyhumans',
  endpoint: 'https://api.tinyhumans.ai',
  has_key: true,
  status: 'ok',
  fetch_modes: ['hybrid'],
};
const CLOUD_ON: EngineState = {
  engine: 'cortexdb',
  endpoint: 'https://api-v1.cortexdb.ai',
  has_key: true,
  status: 'ok',
  fetch_modes: ['hybrid'],
};
const LOCAL_ON: EngineState = { ...CLOUD_ON, endpoint: 'http://localhost:3141' };

function renderTab(state: EngineState | null = OFF) {
  const onStateChange = vi.fn();
  renderWithProviders(<MemoryEngineTab state={state} onStateChange={onStateChange} />);
  return { onStateChange };
}

/** Open a provider's Settings modal and return it. */
const openSettings = (option: string) => {
  fireEvent.click(screen.getByTestId(`memory-engine-${option}-settings`));
  return screen.getByTestId('memory-engine-modal');
};
const type = (testId: string, value: string) =>
  fireEvent.change(screen.getByTestId(testId), { target: { value } });

beforeEach(() => {
  hoisted.engineSet.mockReset();
  hoisted.openUrl.mockReset().mockResolvedValue(undefined);
  hoisted.signedIn = true;
  hoisted.token = 'header.payload.sig';
  hoisted.plan = null;
  hoisted.toastAdd.mockReset();
});

describe('isLoopbackEndpoint', () => {
  it.each([
    ['http://localhost:3141', true],
    ['http://127.0.0.1:3141/', true],
    ['http://127.1.2.3:3141', true],
    ['http://[::1]:3141', true],
    ['https://localhost', true],
    [' http://localhost:3141 ', true],
    ['http://192.168.1.10:3141', false],
    // The URL parser rejects out-of-range octets before the host check runs.
    ['http://127.999.1.1:3141', false],
    ['http://127.256.0.1:3141', false],
    ['http://memory.example.internal:3141/', false],
    ['https://api-v1.cortexdb.ai', false],
    ['http://localhost.evil.com', false],
    ['ftp://localhost', false],
    ['localhost:3141', false],
    ['', false],
  ])('%s → %s', (endpoint, expected) => {
    expect(isLoopbackEndpoint(endpoint)).toBe(expected);
  });
});

describe('MemoryEngineTab', () => {
  it('shows the three providers with a plain prompt when memory is off', () => {
    renderTab({ ...OFF, reason: 'legacy memory backend is unsupported' });
    for (const option of ['builtin', 'apikey', 'selfhost']) {
      expect(screen.getByTestId(`memory-engine-${option}`)).toBeInTheDocument();
    }
    expect(screen.getByText('TinyHumans Memory')).toBeInTheDocument();
    expect(screen.getByText('CortexDB Cloud')).toBeInTheDocument();
    expect(screen.getByText('CortexDB Local')).toBeInTheDocument();
    const banner = screen.getByTestId('memory-engine-status-off');
    expect(banner).toHaveTextContent('Pick a provider below to start remembering.');
    // The core's developer-facing reason is not shown to people.
    expect(banner).not.toHaveTextContent('legacy memory backend');
    expect(screen.getByTestId('memory-engine-builtin-status')).toHaveTextContent('Ready');
    expect(screen.getByTestId('memory-engine-apikey-status')).toHaveTextContent('Not connected');
  });

  it('lists upcoming engines as coming soon, with nothing to click', () => {
    renderTab();
    const soon = screen.getByTestId('memory-engines-soon');
    for (const id of ['supermemory', 'mem0', 'cognee', 'zep', 'letta']) {
      const card = within(soon).getByTestId(`memory-engine-soon-${id}`);
      expect(card).toHaveAttribute('aria-disabled', 'true');
      expect(within(card).getByText('Soon')).toBeInTheDocument();
      expect(within(card).queryByRole('button')).not.toBeInTheDocument();
    }
  });

  describe('TinyHumans', () => {
    it('is used in one click when signed in, and toasts the switch', async () => {
      hoisted.engineSet.mockResolvedValue(BUILTIN_ON);
      const { onStateChange } = renderTab();
      fireEvent.click(screen.getByTestId('memory-engine-builtin-action'));
      await waitFor(() => expect(onStateChange).toHaveBeenCalledWith(BUILTIN_ON));
      expect(hoisted.engineSet).toHaveBeenCalledWith({ engine: 'tinyhumans' });
      expect(hoisted.toastAdd).toHaveBeenCalledWith(
        expect.objectContaining({
          type: 'success',
          title: 'Memory provider switched',
          description: 'TinyHumans Memory now stores your memory.',
        })
      );
    });

    it('shows Connecting while the switch is in flight', () => {
      hoisted.engineSet.mockReturnValue(new Promise(() => undefined));
      renderTab();
      fireEvent.click(screen.getByTestId('memory-engine-builtin-action'));
      expect(screen.getByTestId('memory-engine-builtin-action')).toHaveTextContent('Connecting…');
      expect(screen.getByTestId('memory-engine-builtin-action')).toBeDisabled();
    });

    it('marks an active engine In use with no action button', () => {
      renderTab(BUILTIN_ON);
      expect(screen.getByTestId('memory-engine-builtin')).toHaveAttribute('data-active', 'true');
      expect(screen.getByTestId('memory-engine-builtin-status')).toHaveTextContent('In use');
      expect(screen.queryByTestId('memory-engine-builtin-action')).not.toBeInTheDocument();
      const modal = openSettings('builtin');
      expect(within(modal).queryByTestId('memory-engine-builtin-submit')).not.toBeInTheDocument();
      expect(within(modal).getByTestId('memory-engine-builtin-endpoint')).toHaveTextContent(
        'https://api.tinyhumans.ai'
      );
    });

    it('shows no backend origin while another provider is configured', () => {
      renderTab(CLOUD_ON);
      const modal = openSettings('builtin');
      expect(within(modal).queryByTestId('memory-engine-builtin-endpoint')).not.toBeInTheDocument();
    });

    it('cannot be selected when signed out, and says to sign in', () => {
      hoisted.signedIn = false;
      renderTab();
      expect(screen.getByTestId('memory-engine-builtin-status')).toHaveTextContent(
        'Sign in to use'
      );
      expect(screen.getByTestId('memory-engine-builtin-action')).toBeDisabled();
      const modal = openSettings('builtin');
      expect(within(modal).getByTestId('memory-engine-builtin-sign-in')).toBeInTheDocument();
      expect(within(modal).getByTestId('memory-engine-builtin-submit')).toBeDisabled();
    });

    it('treats a local session token as signed out', () => {
      hoisted.token = createLocalSessionToken();
      renderTab();
      expect(screen.getByTestId('memory-engine-builtin-action')).toBeDisabled();
    });

    it('shows Off on the configured engine while signed out, and can be retried', () => {
      hoisted.signedIn = false;
      renderTab({ ...OFF, engine: 'tinyhumans' });
      expect(screen.getByTestId('memory-engine-builtin-status')).toHaveTextContent('Off');
      expect(screen.getByTestId('memory-engine-builtin-action')).toBeDisabled();
    });

    it('keeps a failed switch inside the modal', async () => {
      hoisted.engineSet.mockRejectedValue(new Error('backend unavailable'));
      renderTab();
      const modal = openSettings('builtin');
      fireEvent.click(within(modal).getByTestId('memory-engine-builtin-submit'));
      expect(await within(modal).findByTestId('memory-engine-builtin-error')).toHaveTextContent(
        'backend unavailable'
      );
      expect(hoisted.toastAdd).not.toHaveBeenCalled();
    });

    it('notes free ingestion by plan and states the fair-use terms', () => {
      hoisted.plan = 'PRO';
      renderTab(BUILTIN_ON);
      expect(screen.getByTestId('memory-engine-builtin-note')).toHaveTextContent(
        'Free memory ingestion on your Pro plan'
      );
      const modal = openSettings('builtin');
      const terms = within(modal).getByTestId('memory-engine-fair-use');
      expect(terms).toHaveTextContent('Your Pro plan includes memory ingestion');
      expect(terms).toHaveTextContent('No automated bulk uploads');
      fireEvent.click(within(terms).getByTestId('memory-engine-terms'));
      expect(hoisted.openUrl).toHaveBeenCalledWith(
        'https://tinyhumans.gitbook.io/openhuman/legal/terms-of-use'
      );
    });

    it('points free plans at Basic and Pro', () => {
      hoisted.plan = 'FREE';
      renderTab();
      expect(screen.getByTestId('memory-engine-builtin-note')).toHaveTextContent(
        'Free memory ingestion on Basic and Pro plans'
      );
      const modal = openSettings('builtin');
      expect(within(modal).getByTestId('memory-engine-fair-use')).toHaveTextContent(
        'Basic and Pro plans include memory ingestion'
      );
    });
  });

  describe('CortexDB Cloud', () => {
    it('connects with only a key, clearing any custom endpoint', async () => {
      hoisted.engineSet.mockResolvedValue(CLOUD_ON);
      const { onStateChange } = renderTab();
      fireEvent.click(screen.getByTestId('memory-engine-apikey-action'));
      const modal = screen.getByTestId('memory-engine-modal');
      const submit = within(modal).getByTestId('memory-engine-apikey-submit');
      expect(submit).toBeDisabled();
      type('memory-engine-apikey-key', 'ck_live_123');
      fireEvent.click(submit);
      await waitFor(() => expect(onStateChange).toHaveBeenCalledWith(CLOUD_ON));
      expect(hoisted.engineSet).toHaveBeenCalledWith({
        engine: 'cortexdb',
        endpoint: '',
        api_key: 'ck_live_123',
      });
      // A successful connect closes the modal.
      expect(screen.queryByTestId('memory-engine-modal')).not.toBeInTheDocument();
    });

    it('is In use for a cloud engine, and saves without a new key', async () => {
      hoisted.engineSet.mockResolvedValue(CLOUD_ON);
      renderTab(CLOUD_ON);
      expect(screen.getByTestId('memory-engine-apikey-status')).toHaveTextContent('In use');
      expect(screen.queryByTestId('memory-engine-apikey-action')).not.toBeInTheDocument();
      const modal = openSettings('apikey');
      expect(within(modal).getByText(/A key is already saved/)).toBeInTheDocument();
      const submit = within(modal).getByTestId('memory-engine-apikey-submit');
      expect(submit).toHaveTextContent('Save');
      fireEvent.click(submit);
      await waitFor(() =>
        expect(hoisted.engineSet).toHaveBeenCalledWith({ engine: 'cortexdb', endpoint: '' })
      );
      expect(hoisted.toastAdd).toHaveBeenCalledWith({
        type: 'success',
        title: 'Memory settings saved',
      });
    });

    it('shows a rejected key inside the modal', async () => {
      hoisted.engineSet.mockRejectedValue(new Error('invalid api key'));
      renderTab();
      const modal = openSettings('apikey');
      type('memory-engine-apikey-key', 'bad');
      fireEvent.click(within(modal).getByTestId('memory-engine-apikey-submit'));
      expect(await within(modal).findByTestId('memory-engine-apikey-error')).toHaveTextContent(
        'invalid api key'
      );
    });

    it('reports a degraded or unreachable engine on its badge and banner', () => {
      const { unmount } = renderWithProviders(
        <MemoryEngineTab
          state={{ ...CLOUD_ON, status: 'degraded', reason: 'slow' }}
          onStateChange={vi.fn()}
        />
      );
      expect(screen.getByTestId('memory-engine-apikey-status')).toHaveTextContent('Degraded');
      expect(screen.getByTestId('memory-engine-status-degraded')).toHaveTextContent('slow');
      unmount();
      renderTab({ ...CLOUD_ON, status: 'down', reason: 'connection refused' });
      expect(screen.getByTestId('memory-engine-apikey-status')).toHaveTextContent('Unreachable');
      expect(screen.getByTestId('memory-engine-status-down')).toHaveTextContent(
        'connection refused'
      );
    });
  });

  describe('CortexDB Local', () => {
    it('connects a loopback server with its key', async () => {
      hoisted.engineSet.mockResolvedValue(LOCAL_ON);
      const { onStateChange } = renderTab();
      fireEvent.click(screen.getByTestId('memory-engine-selfhost-action'));
      const modal = screen.getByTestId('memory-engine-modal');
      type('memory-engine-selfhost-endpoint', 'http://localhost:3141');
      type('memory-engine-selfhost-key', 'local-key');
      fireEvent.click(within(modal).getByTestId('memory-engine-selfhost-submit'));
      await waitFor(() => expect(onStateChange).toHaveBeenCalledWith(LOCAL_ON));
      expect(hoisted.engineSet).toHaveBeenCalledWith({
        engine: 'cortexdb',
        endpoint: 'http://localhost:3141',
        api_key: 'local-key',
      });
    });

    it('refuses an endpoint that is not on this computer', () => {
      renderTab();
      const modal = openSettings('selfhost');
      type('memory-engine-selfhost-endpoint', 'http://192.168.1.10:3141');
      type('memory-engine-selfhost-key', 'k');
      expect(
        within(modal).getByTestId('memory-engine-selfhost-endpoint-error')
      ).toBeInTheDocument();
      expect(within(modal).getByTestId('memory-engine-selfhost-submit')).toBeDisabled();
    });

    it('is In use for a loopback engine, with its endpoint filled in', () => {
      renderTab(LOCAL_ON);
      expect(screen.getByTestId('memory-engine-selfhost-status')).toHaveTextContent('In use');
      const modal = openSettings('selfhost');
      expect(
        (within(modal).getByTestId('memory-engine-selfhost-endpoint') as HTMLInputElement).value
      ).toBe('http://localhost:3141');
    });

    it('fills in the local endpoint when the state arrives after the first render', () => {
      const onStateChange = vi.fn();
      const { rerender } = renderWithProviders(
        <MemoryEngineTab state={OFF} onStateChange={onStateChange} />
      );
      rerender(<MemoryEngineTab state={LOCAL_ON} onStateChange={onStateChange} />);
      const modal = openSettings('selfhost');
      expect(
        (within(modal).getByTestId('memory-engine-selfhost-endpoint') as HTMLInputElement).value
      ).toBe('http://localhost:3141');
    });

    it('links out to the CortexDB self-hosting guide', () => {
      renderTab();
      const modal = openSettings('selfhost');
      fireEvent.click(within(modal).getByTestId('memory-engine-selfhost-docs'));
      expect(hoisted.openUrl).toHaveBeenCalledWith(CORTEXDB_SELF_HOST_DOCS_URL);
    });
  });

  it('closes the modal from its Close button', () => {
    renderTab();
    openSettings('apikey');
    fireEvent.click(screen.getByTestId('memory-engine-modal-close'));
    expect(screen.queryByTestId('memory-engine-modal')).not.toBeInTheDocument();
  });

  it('leaves upcoming engines out when embedded in onboarding', () => {
    renderWithProviders(<MemoryEngineTab state={OFF} onStateChange={vi.fn()} embedded />);
    expect(screen.getByTestId('memory-engines')).toBeInTheDocument();
    expect(screen.queryByTestId('memory-engines-soon')).not.toBeInTheDocument();
  });

  it('shows a loading state until the engine state arrives', () => {
    renderTab(null);
    expect(screen.queryByTestId('memory-engine-tab')).not.toBeInTheDocument();
  });
});
