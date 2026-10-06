import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { LiveVoiceProviders, LiveVoiceSettings } from '../../../services/api/liveVoiceApi';
import { renderWithProviders } from '../../../test/test-utils';
import LiveVoicePanel from './LiveVoicePanel';

const api = vi.hoisted(() => ({
  fetchLiveVoiceProviders: vi.fn(),
  fetchLiveVoiceSettings: vi.fn(),
  updateLiveVoiceSettings: vi.fn(),
  testLiveVoiceProvider: vi.fn(),
  saveLiveVoiceProviderKey: vi.fn(),
  clearLiveVoiceProviderKey: vi.fn(),
}));

vi.mock('../../../services/api/liveVoiceApi', () => api);

const PROVIDERS = (sarvamConfigured = false): LiveVoiceProviders => ({
  default_provider: 'gemini-hosted',
  providers: [
    {
      id: 'gemini-hosted',
      label: 'Gemini (TinyHumans)',
      kind: 'hosted',
      configured: true,
      key_slug: null,
      voices: ['Puck', 'Kore'],
      languages: ['en-US'],
    },
    {
      id: 'elevenlabs-hosted',
      label: 'ElevenLabs (TinyHumans)',
      kind: 'hosted',
      configured: true,
      key_slug: null,
      voices: [],
      languages: [],
    },
    {
      id: 'gemini',
      label: 'Gemini (own key)',
      kind: 'byok',
      configured: true,
      key_slug: 'google',
      voices: [],
      languages: [],
    },
    {
      id: 'sarvam',
      label: 'Sarvam AI',
      kind: 'byok',
      configured: sarvamConfigured,
      key_slug: 'sarvam',
      voices: ['anushka', 'abhilash'],
      languages: ['hi-IN', 'ta-IN'],
    },
  ],
});

const SETTINGS: LiveVoiceSettings = {
  default_provider: 'gemini-hosted',
  gemini: { model: null, voice: 'Puck', language: null },
  sarvam: { language: 'hi-IN', speaker: null, model: null },
  elevenlabs: { voice_id: null },
};

async function renderPanel() {
  renderWithProviders(<LiveVoicePanel />);
  await screen.findByTestId('live-voice-providers');
}

describe('LiveVoicePanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.fetchLiveVoiceProviders.mockResolvedValue(PROVIDERS());
    api.fetchLiveVoiceSettings.mockResolvedValue(SETTINGS);
    api.updateLiveVoiceSettings.mockImplementation(async patch => ({ ...SETTINGS, ...patch }));
    api.saveLiveVoiceProviderKey.mockResolvedValue(undefined);
    api.clearLiveVoiceProviderKey.mockResolvedValue(undefined);
  });

  it('renders a card per provider with kind and readiness badges', async () => {
    await renderPanel();
    for (const id of ['gemini-hosted', 'elevenlabs-hosted', 'gemini', 'sarvam']) {
      expect(screen.getByTestId(`live-voice-provider-${id}`)).toBeInTheDocument();
    }
    const hosted = screen.getByTestId('live-voice-provider-gemini-hosted');
    expect(within(hosted).getByText('Included')).toBeInTheDocument();
    expect(within(hosted).getByText('Default')).toBeInTheDocument();
    expect(hosted).toHaveAttribute('data-selected', 'true');
    const sarvam = screen.getByTestId('live-voice-provider-sarvam');
    expect(within(sarvam).getByText('Your key')).toBeInTheDocument();
    expect(within(sarvam).getByText('Needs a key')).toBeInTheDocument();
    // Hosted cards have no key entry.
    expect(screen.queryByTestId('live-voice-key-gemini-hosted')).not.toBeInTheDocument();
    // An unconfigured BYOK provider cannot be tested yet.
    expect(screen.getByTestId('live-voice-test-button-sarvam')).toBeDisabled();
  });

  it('lists unconfigured providers as disabled default choices and saves a new default', async () => {
    await renderPanel();
    const select = screen.getByTestId('live-voice-default') as HTMLSelectElement;
    expect(select.value).toBe('gemini-hosted');
    const sarvamOption = within(select).getByRole('option', { name: 'Sarvam AI (needs a key)' });
    expect(sarvamOption).toBeDisabled();

    fireEvent.change(select, { target: { value: 'gemini' } });
    await waitFor(() =>
      expect(api.updateLiveVoiceSettings).toHaveBeenCalledWith({ default_provider: 'gemini' })
    );
    await screen.findByText('Saved.');
    expect(within(screen.getByTestId('live-voice-provider-gemini')).getByText('Default')).toBeTruthy();
  });

  it('saves a BYOK key and re-fetches providers', async () => {
    await renderPanel();
    const save = screen.getByTestId('live-voice-save-key-sarvam');
    expect(save).toBeDisabled();
    api.fetchLiveVoiceProviders.mockResolvedValueOnce(PROVIDERS(true));

    fireEvent.change(screen.getByTestId('live-voice-key-sarvam'), {
      target: { value: 'sk-sarvam' },
    });
    fireEvent.click(save);

    await waitFor(() =>
      expect(api.saveLiveVoiceProviderKey).toHaveBeenCalledWith('sarvam', 'sk-sarvam')
    );
    await screen.findByText('Key saved.');
    expect(api.fetchLiveVoiceProviders).toHaveBeenCalledTimes(2);
    const sarvam = screen.getByTestId('live-voice-provider-sarvam');
    expect(within(sarvam).getByText('Ready')).toBeInTheDocument();
    expect((screen.getByTestId('live-voice-key-sarvam') as HTMLInputElement).value).toBe('');
  });

  it('removes a stored key', async () => {
    await renderPanel();
    fireEvent.click(screen.getByTestId('live-voice-clear-key-gemini'));
    await waitFor(() => expect(api.clearLiveVoiceProviderKey).toHaveBeenCalledWith('google'));
    await screen.findByText('Key removed.');
  });

  it('surfaces a key-save failure', async () => {
    api.saveLiveVoiceProviderKey.mockRejectedValueOnce(new Error('keyring locked'));
    await renderPanel();
    fireEvent.change(screen.getByTestId('live-voice-key-gemini'), { target: { value: 'k' } });
    fireEvent.click(screen.getByTestId('live-voice-save-key-gemini'));
    await screen.findByText(/Couldn't save: keyring locked/);
  });

  it('surfaces a key-removal failure', async () => {
    api.clearLiveVoiceProviderKey.mockRejectedValueOnce(new Error('nope'));
    await renderPanel();
    fireEvent.click(screen.getByTestId('live-voice-clear-key-gemini'));
    await screen.findByText(/Couldn't save: nope/);
  });

  it('runs a provider test and shows latency, then a failure', async () => {
    api.testLiveVoiceProvider.mockResolvedValueOnce({ ok: true, latency_ms: 240, error: null });
    await renderPanel();
    fireEvent.click(screen.getByTestId('live-voice-test-button-gemini-hosted'));
    expect(api.testLiveVoiceProvider).toHaveBeenCalledWith('gemini-hosted');
    const line = await screen.findByText('Working · 240 ms');
    expect(line).toHaveAttribute('data-ok', 'true');

    api.testLiveVoiceProvider.mockResolvedValueOnce({ ok: true, latency_ms: null, error: null });
    fireEvent.click(screen.getByTestId('live-voice-test-button-elevenlabs-hosted'));
    await screen.findByText('Working');

    api.testLiveVoiceProvider.mockResolvedValueOnce({ ok: false, latency_ms: null, error: 'bad key' });
    fireEvent.click(screen.getByTestId('live-voice-test-button-gemini'));
    await screen.findByText('Test failed: bad key');

    api.testLiveVoiceProvider.mockRejectedValueOnce(new Error('timeout'));
    fireEvent.click(screen.getByTestId('live-voice-test-button-gemini-hosted'));
    await screen.findByText('Test failed: timeout');
  });

  it('shows a testing state while the probe is in flight', async () => {
    let finish: (v: unknown) => void = () => undefined;
    api.testLiveVoiceProvider.mockReturnValueOnce(new Promise(r => (finish = r)));
    await renderPanel();
    fireEvent.click(screen.getByTestId('live-voice-test-button-gemini'));
    expect(screen.getByTestId('live-voice-test-button-gemini')).toBeDisabled();
    expect(screen.getByTestId('live-voice-test-gemini')).toHaveTextContent('Testing…');
    finish({ ok: true, latency_ms: 5, error: null });
    await screen.findByText('Working · 5 ms');
  });

  it('writes Sarvam speaker and language and Gemini voice to their settings blocks', async () => {
    await renderPanel();
    expect((screen.getByTestId('live-voice-voice-gemini-hosted') as HTMLSelectElement).value).toBe(
      'Puck'
    );
    expect((screen.getByTestId('live-voice-language-sarvam') as HTMLSelectElement).value).toBe(
      'hi-IN'
    );
    // No pickers for a provider that lists no voices or languages.
    expect(screen.queryByTestId('live-voice-voice-elevenlabs-hosted')).not.toBeInTheDocument();

    fireEvent.change(screen.getByTestId('live-voice-voice-sarvam'), {
      target: { value: 'anushka' },
    });
    await waitFor(() =>
      expect(api.updateLiveVoiceSettings).toHaveBeenCalledWith({ sarvam: { speaker: 'anushka' } })
    );
    fireEvent.change(screen.getByTestId('live-voice-language-sarvam'), { target: { value: '' } });
    await waitFor(() =>
      expect(api.updateLiveVoiceSettings).toHaveBeenCalledWith({ sarvam: { language: null } })
    );
    fireEvent.change(screen.getByTestId('live-voice-language-gemini-hosted'), {
      target: { value: 'en-US' },
    });
    await waitFor(() =>
      expect(api.updateLiveVoiceSettings).toHaveBeenCalledWith({ gemini: { language: 'en-US' } })
    );
  });

  it('shows a save error', async () => {
    api.updateLiveVoiceSettings.mockRejectedValueOnce(new Error('disk full'));
    await renderPanel();
    fireEvent.change(screen.getByTestId('live-voice-default'), { target: { value: 'gemini' } });
    await screen.findByText(/Couldn't save: disk full/);
  });

  it('shows a load error', async () => {
    api.fetchLiveVoiceProviders.mockRejectedValueOnce(new Error('core down'));
    renderWithProviders(<LiveVoicePanel />);
    await screen.findByText(/Couldn't load voice agent settings: core down/);
    expect(screen.queryByTestId('live-voice-providers')).not.toBeInTheDocument();
  });
});
