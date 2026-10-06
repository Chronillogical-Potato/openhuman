import { Mic, Play, Volume2 } from 'lucide-react';
import { type ReactNode, useState } from 'react';

import { testVoiceProvider, type VoiceProviderView } from '../../../services/api/voiceSettingsApi';
import type { VoiceStatus } from '../../../utils/tauriCommands';
import { Badge, Button, Card, Field, NativeSelect, TextField } from '../../ui';
import { Spinner } from '../../ui/icons';
import { ELEVENLABS_VOICE_PRESETS, isCuratedVoicePreset } from './elevenlabsVoicePresets';

interface VoicePanelRoutingSectionProps {
  t: (key: string) => string;
  sttProvider: string;
  ttsProvider: string;
  onSttProviderChange: (next: string) => void;
  onTtsProviderChange: (next: string) => void;
  isSavingProviders: boolean;
  sttExternalProviders: VoiceProviderView[];
  ttsExternalProviders: VoiceProviderView[];
  piperEnabledElsewhere: boolean;
  ttsVoice: string;
  setTtsVoice: (value: string) => void;
  piperVoicePresets: ReadonlyArray<{ id: string; label: string }>;
  piperVoicePresetIds: readonly string[];
  voiceStatus: VoiceStatus | null;
  persistProviders: (update: { tts_voice?: string }) => Promise<void>;
  elevenlabsVoiceId: string;
  setElevenlabsVoiceId: (value: string) => void;
  ttsTestBlockedByInstall: boolean;
  hasRoutingChanges: boolean;
  isSavingRouting: boolean;
  saveRouting: () => Promise<void>;
}

type TestResult = { ok: boolean; detail: string } | null;

/** A workload row (speech to text / text to speech): icon + label, then its controls. */
const WorkloadRow = ({
  icon,
  label,
  control,
}: {
  icon: ReactNode;
  label: string;
  control: ReactNode;
}) => (
  <div className="flex flex-wrap items-center justify-between gap-3 px-4 py-3">
    <span className="flex items-center gap-2.5 text-sm font-medium text-content">
      <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-surface-muted text-content-secondary">
        {icon}
      </span>
      {label}
    </span>
    {control}
  </div>
);

/** Outcome of a Test click: a status chip plus the provider's detail text. */
const TestResultLine = ({ result, t }: { result: TestResult; t: (key: string) => string }) =>
  result ? (
    <div className="flex items-center gap-2 px-4 pb-3 text-xs text-content-muted">
      <Badge variant={result.ok ? 'success' : 'danger'}>
        {result.ok ? t('voice.routing.testOk') : t('voice.routing.testFailed')}
      </Badge>
      <span className="min-w-0 truncate" title={result.detail}>
        {result.detail}
      </span>
    </div>
  ) : null;

/** STT/TTS provider routing pickers + per-workload test buttons. */
const VoicePanelRoutingSection = ({
  t,
  sttProvider,
  ttsProvider,
  onSttProviderChange,
  onTtsProviderChange,
  isSavingProviders,
  sttExternalProviders,
  ttsExternalProviders,
  piperEnabledElsewhere,
  ttsVoice,
  setTtsVoice,
  piperVoicePresets,
  piperVoicePresetIds,
  voiceStatus,
  persistProviders,
  elevenlabsVoiceId,
  setElevenlabsVoiceId,
  ttsTestBlockedByInstall,
  hasRoutingChanges,
  isSavingRouting,
  saveRouting,
}: VoicePanelRoutingSectionProps) => {
  const [isTestingStt, setIsTestingStt] = useState(false);
  const [sttTestResult, setSttTestResult] = useState<TestResult>(null);
  const [isTestingTts, setIsTestingTts] = useState(false);
  const [ttsTestResult, setTtsTestResult] = useState<TestResult>(null);

  const runSttTest = async () => {
    setIsTestingStt(true);
    setSttTestResult(null);
    try {
      setSttTestResult(await testVoiceProvider('stt', sttProvider || 'cloud'));
    } catch (err) {
      setSttTestResult({ ok: false, detail: err instanceof Error ? err.message : 'Test failed' });
    } finally {
      setIsTestingStt(false);
    }
  };

  const runTtsTest = async () => {
    setIsTestingTts(true);
    setTtsTestResult(null);
    try {
      // For ElevenLabs, include the voice ID so the test actually synthesizes
      // audio with the selected voice.
      let ttsTestProvider = ttsProvider || 'cloud';
      if (ttsProvider === 'elevenlabs' && elevenlabsVoiceId) {
        ttsTestProvider = `elevenlabs:${elevenlabsVoiceId}`;
      }
      setTtsTestResult(await testVoiceProvider('tts', ttsTestProvider));
    } catch (err) {
      setTtsTestResult({ ok: false, detail: err instanceof Error ? err.message : 'Test failed' });
    } finally {
      setIsTestingTts(false);
    }
  };

  const piperVoiceIsPreset = piperVoicePresetIds.some(v => v === ttsVoice);

  return (
    <Card title={t('voice.routing.title')} description={t('voice.routing.desc')}>
      {/* ── Speech to text ─────────────────────────────────────────── */}
      <div>
        <WorkloadRow
          icon={<Mic className="h-4 w-4" aria-hidden />}
          label={t('voice.providers.sttProvider')}
          control={
            <div className="flex items-center gap-2">
              <NativeSelect
                aria-label={t('voice.providers.sttProviderAria')}
                data-testid="stt-provider-select"
                value={sttProvider || 'cloud'}
                disabled={isSavingProviders}
                onChange={e => onSttProviderChange(e.target.value)}
                inputSize="sm"
                className="w-56">
                <option value="cloud">{t('voice.providers.backendSttProxy')}</option>
                {sttExternalProviders.map(p => (
                  <option key={p.slug} value={p.slug}>
                    {p.label}
                  </option>
                ))}
              </NativeSelect>
              <Button
                type="button"
                variant="secondary"
                size="sm"
                data-testid="test-stt-button"
                leadingIcon={
                  isTestingStt ? <Spinner /> : <Play className="h-3.5 w-3.5" aria-hidden />
                }
                disabled={isTestingStt || !sttProvider}
                onClick={() => void runSttTest()}>
                {isTestingStt ? t('voice.modal.testing') : t('voice.routing.testStt')}
              </Button>
            </div>
          }
        />
        <TestResultLine result={sttTestResult} t={t} />
      </div>

      {/* ── Text to speech ─────────────────────────────────────────── */}
      <div>
        <WorkloadRow
          icon={<Volume2 className="h-4 w-4" aria-hidden />}
          label={t('voice.providers.ttsProvider')}
          control={
            <div className="flex items-center gap-2">
              <NativeSelect
                aria-label={t('voice.providers.ttsProviderAria')}
                data-testid="tts-provider-select"
                value={ttsProvider || 'cloud'}
                disabled={isSavingProviders}
                onChange={e => onTtsProviderChange(e.target.value)}
                inputSize="sm"
                className="w-56">
                <option value="cloud">{t('voice.providers.cloudElevenLabsProxy')}</option>
                {/* Piper only shown when enabled */}
                {(ttsProvider === 'piper' || piperEnabledElsewhere) && (
                  <option value="piper">{t('voice.providers.localPiper')}</option>
                )}
                {ttsExternalProviders.map(p => (
                  <option key={p.slug} value={p.slug}>
                    {p.label}
                  </option>
                ))}
              </NativeSelect>
              <Button
                type="button"
                variant="secondary"
                size="sm"
                data-testid="test-tts-button"
                leadingIcon={
                  isTestingTts ? <Spinner /> : <Play className="h-3.5 w-3.5" aria-hidden />
                }
                disabled={isTestingTts || !ttsProvider || ttsTestBlockedByInstall}
                title={ttsTestBlockedByInstall ? t('voice.providers.piperNotFound') : undefined}
                onClick={() => void runTtsTest()}>
                {isTestingTts ? t('voice.modal.testing') : t('voice.routing.testTts')}
              </Button>
            </div>
          }
        />
        <TestResultLine result={ttsTestResult} t={t} />
      </div>

      {/* Piper voice picker — shown when Piper is selected */}
      {ttsProvider === 'piper' && (
        <Field
          label={t('voice.providers.piperVoice')}
          description={t('voice.providers.piperVoicesDesc')}
          control={
            <div className="flex w-72 flex-col gap-2">
              <NativeSelect
                aria-label={t('voice.providers.piperVoiceAria')}
                data-testid="tts-voice-select"
                value={piperVoiceIsPreset ? ttsVoice : '__custom__'}
                disabled={isSavingProviders}
                inputSize="sm"
                onChange={e => {
                  const next = e.target.value;
                  if (next === '__custom__') return;
                  setTtsVoice(next);
                  void persistProviders({ tts_voice: next });
                }}
                className="w-full">
                {piperVoicePresets.map(v => (
                  <option key={v.id} value={v.id}>
                    {v.label}
                  </option>
                ))}
                <option value="__custom__">{t('voice.providers.customVoiceOption')}</option>
              </NativeSelect>
              {!piperVoiceIsPreset && (
                <TextField
                  aria-label={t('voice.providers.customVoiceAria')}
                  data-testid="tts-voice-input"
                  mono
                  inputSize="sm"
                  value={ttsVoice}
                  placeholder={t('voice.providers.customVoicePlaceholder')}
                  disabled={isSavingProviders}
                  onChange={e => setTtsVoice(e.target.value)}
                  onBlur={() => {
                    if (ttsVoice && ttsVoice !== voiceStatus?.tts_voice_id) {
                      void persistProviders({ tts_voice: ttsVoice });
                    }
                  }}
                  className="w-full"
                />
              )}
            </div>
          }
        />
      )}

      {/* ElevenLabs voice picker — shown when ElevenLabs is selected for TTS */}
      {ttsProvider === 'elevenlabs' && (
        <Field
          label={t('voice.routing.elevenlabsVoice')}
          description={t('voice.routing.elevenlabsVoiceDesc')}
          control={
            <div className="flex w-72 flex-col gap-2">
              <NativeSelect
                aria-label={t('voice.routing.elevenlabsVoiceAria')}
                data-testid="elevenlabs-voice-select"
                value={isCuratedVoicePreset(elevenlabsVoiceId) ? elevenlabsVoiceId : '__custom__'}
                disabled={isSavingProviders}
                inputSize="sm"
                onChange={e => {
                  const next = e.target.value;
                  if (next === '__custom__') return;
                  setElevenlabsVoiceId(next);
                }}
                className="w-full">
                {ELEVENLABS_VOICE_PRESETS.map(v => (
                  <option key={v.id} value={v.id}>
                    {v.label}
                  </option>
                ))}
                <option value="__custom__">{t('voice.providers.customVoiceOption')}</option>
              </NativeSelect>
              {!isCuratedVoicePreset(elevenlabsVoiceId) && (
                <TextField
                  aria-label={t('voice.routing.elevenlabsVoiceIdAria')}
                  data-testid="elevenlabs-voice-input"
                  mono
                  inputSize="sm"
                  value={elevenlabsVoiceId}
                  placeholder="JBFqnCBsd6RMkjVDRZzb"
                  disabled={isSavingProviders}
                  onChange={e => setElevenlabsVoiceId(e.target.value)}
                  className="w-full"
                />
              )}
            </div>
          }
        />
      )}

      <div className="flex items-center justify-end gap-3 px-4 py-3">
        {hasRoutingChanges && (
          <span className="text-xs text-content-muted">{t('voice.routing.unsaved')}</span>
        )}
        <Button
          type="button"
          variant="primary"
          size="sm"
          data-testid="save-voice-routing"
          disabled={!hasRoutingChanges || isSavingRouting}
          onClick={() => void saveRouting()}>
          {isSavingRouting ? t('common.loading') : t('voice.routing.save')}
        </Button>
      </div>
    </Card>
  );
};

export default VoicePanelRoutingSection;
