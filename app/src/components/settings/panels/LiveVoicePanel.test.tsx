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

  it('groups providers into included and own-key sections with readiness', async () => {
    await renderPanel();
    const hostedGroup = screen.getByTestId('live-voice-group-hosted');
    const byokGroup = screen.getByTestId('live-voice-group-byok');
    expect(within(hostedGroup).getByText('Included with OpenHuman')).toBeInTheDocument();
    expect(within(byokGroup).getByText('Use your own key')).toBeInTheDocument();
    for (const id of ['gemini-hosted', 'elevenlabs-hosted']) {
      expect(within(hostedGroup).getByTestId(`live-voice-provider-${id}`)).toBeInTheDocument();
    }
    for (const id of ['gemini', 'sarvam']) {
      expect(within(byokGroup).getByTestId(`live-voice-provider-${id}`)).toBeInTheDocument();
    }
    const hosted = screen.getByTestId('live-voice-provider-gemini-hosted');
    expect(within(hosted).getByText('In use')).toBeInTheDocument();
    expect(hosted).toHaveAttribute('data-selected', 'true');
    const sarvam = screen.getByTestId('live-voice-provider-sarvam');
    expect(within(sarvam).getByText('Needs a key')).toBeInTheDocument();
    expect(within(sarvam).getByText('Add an API key to use this agent.')).toBeInTheDocument();
    // Hosted cards have no key entry.
    expect(screen.queryByTestId('live-voice-key-gemini-hosted')).not.toBeInTheDocument();
    // An unconfigured BYOK provider cannot be tested yet.
    expect(screen.queryByTestId('live-voice-test-button-sarvam')).not.toBeInTheDocument();
  });

  it('picks the agent in use from its row and refuses one without a key', async () => {
    await renderPanel();
    const current = screen.getByTestId('live-voice-default-gemini-hosted') as HTMLInputElement;
    expect(current.checked).toBe(true);
    expect(screen.getByTestId('live-voice-default-sarvam')).toBeDisabled();

    fireEvent.click(screen.getByTestId('live-voice-default-gemini'));
    await waitFor(() =>
      expect(api.updateLiveVoiceSettings).toHaveBeenCalledWith({ default_provider: 'gemini' })
    );
    await screen.findByText('Saved.');
    const gemini = screen.getByTestId('live-voice-provider-gemini');
    expect(gemini).toHaveAttribute('data-selected', 'true');
    expect(within(gemini).getByText('In use')).toBeInTheDocument();
    expect(screen.getByTestId('live-voice-provider-gemini-hosted')).not.toHaveAttribute(
      'data-selected'
    );
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
    // A stored key is replaced through an explicit Replace step.
    expect(screen.queryByTestId('live-voice-key-gemini')).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId('live-voice-replace-key-gemini'));
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

    api.testLiveVoiceProvider.mockResolvedValueOnce({
      ok: false,
      latency_ms: null,
      error: 'bad key',
    });
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

  it('shows voice pickers only on the agent in use', async () => {
    await renderPanel();
    expect((screen.getByTestId('live-voice-voice-gemini-hosted') as HTMLSelectElement).value).toBe(
      'Puck'
    );
    expect(screen.queryByTestId('live-voice-voice-sarvam')).not.toBeInTheDocument();
    fireEvent.change(screen.getByTestId('live-voice-language-gemini-hosted'), {
      target: { value: 'en-US' },
    });
    await waitFor(() =>
      expect(api.updateLiveVoiceSettings).toHaveBeenCalledWith({ gemini: { language: 'en-US' } })
    );
  });

  it('writes Sarvam speaker and language to their settings block', async () => {
    api.fetchLiveVoiceProviders.mockResolvedValue({
      ...PROVIDERS(true),
      default_provider: 'sarvam',
    });
    api.fetchLiveVoiceSettings.mockResolvedValue({ ...SETTINGS, default_provider: 'sarvam' });
    await renderPanel();
    expect((screen.getByTestId('live-voice-language-sarvam') as HTMLSelectElement).value).toBe(
      'hi-IN'
    );

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
  });

  it('hides pickers for an agent in use that lists no voices or languages', async () => {
    api.fetchLiveVoiceSettings.mockResolvedValue({
      ...SETTINGS,
      default_provider: 'elevenlabs-hosted',
    });
    await renderPanel();
    expect(screen.getByTestId('live-voice-provider-elevenlabs-hosted')).toHaveAttribute(
      'data-selected',
      'true'
    );
    expect(screen.queryByTestId('live-voice-voice-elevenlabs-hosted')).not.toBeInTheDocument();
  });

  it('shows a save error', async () => {
    api.updateLiveVoiceSettings.mockRejectedValueOnce(new Error('disk full'));
    await renderPanel();
    fireEvent.click(screen.getByTestId('live-voice-default-gemini'));
    await screen.findByText(/Couldn't save: disk full/);
  });

  it('shows a load error', async () => {
    api.fetchLiveVoiceProviders.mockRejectedValueOnce(new Error('core down'));
    renderWithProviders(<LiveVoicePanel />);
    await screen.findByText(/Couldn't load voice agent settings: core down/);
    expect(screen.queryByTestId('live-voice-providers')).not.toBeInTheDocument();
  });
});
