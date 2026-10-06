import { fireEvent, screen, waitFor } from '@testing-library/react';
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
}));

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

const open = (option: string) =>
  fireEvent.click(screen.getByTestId(`memory-engine-${option}-trigger`));
const type = (testId: string, value: string) =>
  fireEvent.change(screen.getByTestId(testId), { target: { value } });

beforeEach(() => {
  hoisted.engineSet.mockReset();
  hoisted.openUrl.mockReset().mockResolvedValue(undefined);
  hoisted.signedIn = true;
  hoisted.token = 'header.payload.sig';
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
  it('shows a loading state until the engine state arrives', () => {
    renderTab(null);
    expect(screen.queryByTestId('memory-engines')).not.toBeInTheDocument();
  });

  it('offers the three options with Built-in open when nothing is connected', () => {
    renderTab();
    expect(screen.getByTestId('memory-engine-builtin')).toBeInTheDocument();
    expect(screen.getByTestId('memory-engine-apikey')).toBeInTheDocument();
    expect(screen.getByTestId('memory-engine-selfhost')).toBeInTheDocument();
    expect(screen.getByTestId('memory-engine-status-off')).toHaveTextContent('Memory is off');
    expect(screen.getByTestId('memory-engine-builtin-use')).toBeEnabled();
    expect(screen.queryByTestId('memory-engine-apikey-key')).not.toBeInTheDocument();
  });

  it('shows the engine-reported reason when memory is off', () => {
    renderTab({ ...OFF, reason: 'Signed out and no CortexDB key' });
    expect(screen.getByTestId('memory-engine-status-off')).toHaveTextContent(
      'Signed out and no CortexDB key'
    );
  });

  describe('Built-in', () => {
    it('selects the tinyhumans engine in one click when signed in', async () => {
      hoisted.engineSet.mockResolvedValue(BUILTIN_ON);
      const { onStateChange } = renderTab();
      fireEvent.click(screen.getByTestId('memory-engine-builtin-use'));
      await waitFor(() => expect(hoisted.engineSet).toHaveBeenCalledWith({ engine: 'tinyhumans' }));
      expect(onStateChange).toHaveBeenCalledWith(BUILTIN_ON);
    });

    it('says to sign in and cannot be selected when signed out', () => {
      hoisted.signedIn = false;
      renderTab();
      expect(screen.getByTestId('memory-engine-builtin-trigger')).toHaveTextContent(
        'Sign in to use'
      );
      expect(screen.getByTestId('memory-engine-builtin-sign-in')).toBeInTheDocument();
      expect(screen.getByTestId('memory-engine-builtin-use')).toBeDisabled();
    });

    it('treats a local session token as signed out', () => {
      hoisted.token = createLocalSessionToken();
      renderTab();
      expect(screen.getByTestId('memory-engine-builtin-use')).toBeDisabled();
    });

    it('marks an active Built-in engine and offers no button', () => {
      renderTab(BUILTIN_ON);
      expect(screen.getByTestId('memory-engine-builtin-active')).toHaveTextContent('Active');
      expect(screen.queryByTestId('memory-engine-builtin-use')).not.toBeInTheDocument();
      expect(screen.queryByTestId('memory-engine-apikey-active')).not.toBeInTheDocument();
    });

    it('shows Off on the configured Built-in engine while signed out', () => {
      hoisted.signedIn = false;
      renderTab({
        ...OFF,
        engine: 'tinyhumans',
        endpoint: 'https://api.tinyhumans.ai',
        reason: 'sign in to use TinyHumans memory',
      });
      expect(screen.getByTestId('memory-engine-builtin-active')).toHaveTextContent('Off');
      expect(screen.getByTestId('memory-engine-builtin-use')).toBeDisabled();
      expect(screen.getByTestId('memory-engine-builtin-sign-in')).toBeInTheDocument();
      expect(screen.getByTestId('memory-engine-builtin-endpoint')).toHaveTextContent(
        'https://api.tinyhumans.ai'
      );
    });

    it('shows the backend origin the core reports, read-only', () => {
      renderTab({ ...BUILTIN_ON, endpoint: 'https://staging-api.example.test' });
      const endpoint = screen.getByTestId('memory-engine-builtin-endpoint');
      expect(endpoint).toHaveTextContent('https://staging-api.example.test');
      expect(endpoint.tagName).not.toBe('INPUT');
      expect(
        screen.getByText(/Facts and beliefs drawn from them fill in over the following minutes/)
      ).toBeInTheDocument();
    });

    it('shows no Built-in origin while another option is configured', () => {
      renderTab(CLOUD_ON);
      open('builtin');
      expect(screen.getByTestId('memory-engine-builtin-use')).toBeInTheDocument();
      expect(screen.queryByTestId('memory-engine-builtin-endpoint')).not.toBeInTheDocument();
    });

    it('shows a failed switch inside the item and stays open', async () => {
      hoisted.engineSet.mockRejectedValue(new Error('UNAUTHORIZED: session expired'));
      renderTab();
      fireEvent.click(screen.getByTestId('memory-engine-builtin-use'));
      expect(await screen.findByTestId('memory-engine-builtin-error')).toHaveTextContent(
        'session expired'
      );
      expect(screen.getByTestId('memory-engine-builtin-use')).toBeEnabled();
    });

    it('shows Connecting while the switch is in flight', async () => {
      let resolve: (s: EngineState) => void = () => undefined;
      hoisted.engineSet.mockReturnValue(new Promise<EngineState>(r => (resolve = r)));
      renderTab();
      fireEvent.click(screen.getByTestId('memory-engine-builtin-use'));
      expect(await screen.findByText('Connecting…')).toBeInTheDocument();
      expect(screen.getByTestId('memory-engine-builtin-use')).toBeDisabled();
      resolve(BUILTIN_ON);
      await waitFor(() => expect(screen.queryByText('Connecting…')).not.toBeInTheDocument());
    });
  });

  describe('API key', () => {
    it('connects CortexDB cloud with only a key, clearing any custom endpoint', async () => {
      hoisted.engineSet.mockResolvedValue(CLOUD_ON);
      const { onStateChange } = renderTab();
      open('apikey');
      expect(screen.getByTestId('memory-engine-apikey-trigger')).toHaveTextContent(
        'https://api-v1.cortexdb.ai'
      );
      expect(screen.queryByTestId('memory-engine-apikey-endpoint')).not.toBeInTheDocument();
      const submit = screen.getByTestId('memory-engine-apikey-submit');
      expect(submit).toBeDisabled();
      type('memory-engine-apikey-key', ' secret ');
      fireEvent.click(submit);

      await waitFor(() =>
        expect(hoisted.engineSet).toHaveBeenCalledWith({
          engine: 'cortexdb',
          endpoint: '',
          api_key: 'secret',
        })
      );
      expect(onStateChange).toHaveBeenCalledWith(CLOUD_ON);
      await waitFor(() => expect(screen.getByTestId('memory-engine-apikey-key')).toHaveValue(''));
    });

    it('is open and active for a cloud cortexdb engine, and saves without a new key', async () => {
      hoisted.engineSet.mockResolvedValue(CLOUD_ON);
      renderTab(CLOUD_ON);
      expect(screen.getByTestId('memory-engine-apikey-active')).toHaveTextContent('Active');
      expect(screen.getByTestId('memory-engine-apikey-key')).toHaveAttribute(
        'placeholder',
        'Saved. Enter a new key to replace it'
      );
      const submit = screen.getByTestId('memory-engine-apikey-submit');
      expect(submit).toHaveTextContent('Save');
      fireEvent.click(submit);
      await waitFor(() =>
        expect(hoisted.engineSet).toHaveBeenCalledWith({ engine: 'cortexdb', endpoint: '' })
      );
    });

    it('reports a degraded or unreachable engine on its badge', () => {
      renderTab({ ...CLOUD_ON, status: 'degraded', reason: 'slow answers' });
      expect(screen.getByTestId('memory-engine-apikey-active')).toHaveTextContent('Degraded');
      expect(screen.getByTestId('memory-engine-status-degraded')).toHaveTextContent('slow answers');
    });

    it('reports an unreachable engine', () => {
      renderTab({ ...CLOUD_ON, status: 'down', reason: 'connection refused' });
      expect(screen.getByTestId('memory-engine-apikey-active')).toHaveTextContent('Unreachable');
      expect(screen.getByTestId('memory-engine-status-down')).toHaveTextContent(
        'connection refused'
      );
    });

    it('shows a rejected key inside the item', async () => {
      hoisted.engineSet.mockRejectedValue(new Error('UNAUTHORIZED: bad key'));
      renderTab();
      open('apikey');
      type('memory-engine-apikey-key', 'wrong');
      fireEvent.click(screen.getByTestId('memory-engine-apikey-submit'));
      expect(await screen.findByTestId('memory-engine-apikey-error')).toHaveTextContent('bad key');
    });
  });

  describe('Self-host', () => {
    it('links out to the CortexDB self-hosting guide', () => {
      renderTab();
      open('selfhost');
      fireEvent.click(screen.getByTestId('memory-engine-selfhost-docs'));
      expect(hoisted.openUrl).toHaveBeenCalledWith(CORTEXDB_SELF_HOST_DOCS_URL);
    });

    it('refuses an endpoint that is not on this computer', () => {
      renderTab();
      open('selfhost');
      type('memory-engine-selfhost-endpoint', 'http://192.168.1.10:3141');
      type('memory-engine-selfhost-key', 'local-key');
      expect(screen.getByTestId('memory-engine-selfhost-endpoint-error')).toHaveTextContent(
        'Self-hosting is local only'
      );
      expect(screen.getByTestId('memory-engine-selfhost-submit')).toBeDisabled();
    });

    it('connects a loopback server with its key', async () => {
      hoisted.engineSet.mockResolvedValue({ ...LOCAL_ON, endpoint: 'http://127.0.0.1:3141' });
      renderTab();
      open('selfhost');
      type('memory-engine-selfhost-endpoint', ' http://127.0.0.1:3141 ');
      expect(screen.getByTestId('memory-engine-selfhost-submit')).toBeDisabled(); // key required
      expect(screen.queryByTestId('memory-engine-selfhost-endpoint-error')).not.toBeInTheDocument();
      type('memory-engine-selfhost-key', 'local-key');
      fireEvent.click(screen.getByTestId('memory-engine-selfhost-submit'));
      await waitFor(() =>
        expect(hoisted.engineSet).toHaveBeenCalledWith({
          engine: 'cortexdb',
          endpoint: 'http://127.0.0.1:3141',
          api_key: 'local-key',
        })
      );
    });

    it('is open and active for a loopback cortexdb engine, with its endpoint filled in', async () => {
      hoisted.engineSet.mockResolvedValue(LOCAL_ON);
      renderTab(LOCAL_ON);
      expect(screen.getByTestId('memory-engine-selfhost-active')).toHaveTextContent('Active');
      expect(screen.queryByTestId('memory-engine-apikey-active')).not.toBeInTheDocument();
      expect(screen.getByTestId('memory-engine-selfhost-trigger')).toHaveTextContent(
        'http://localhost:3141'
      );
      expect(screen.getByTestId('memory-engine-selfhost-endpoint')).toHaveValue(
        'http://localhost:3141'
      );
      fireEvent.click(screen.getByTestId('memory-engine-selfhost-submit'));
      await waitFor(() =>
        expect(hoisted.engineSet).toHaveBeenCalledWith({
          engine: 'cortexdb',
          endpoint: 'http://localhost:3141',
        })
      );
    });

    it('fills in the local endpoint when the state arrives after the first render', () => {
      const onStateChange = vi.fn();
      const { rerender } = renderWithProviders(
        <MemoryEngineTab state={null} onStateChange={onStateChange} />
      );
      rerender(<MemoryEngineTab state={LOCAL_ON} onStateChange={onStateChange} />);
      expect(screen.getByTestId('memory-engine-selfhost-endpoint')).toHaveValue(
        'http://localhost:3141'
      );
    });
  });
});
