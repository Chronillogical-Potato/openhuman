import { useRef, useState } from 'react';

import { testVoiceProvider } from '../../../services/api/voiceSettingsApi';
import { Alert, Button, ModalShell } from '../../ui';
import { SettingsSelect, SettingsTextField } from '../controls';
import { BUILTIN_VOICE_PROVIDER_META } from './VoicePanelProviderChips';

interface VoicePanelKeyModalProps {
  t: (key: string) => string;
  pendingKeySlug: string;
  setPendingKeySlug: (slug: string | null) => void;
  pendingKeyValue: string;
  setPendingKeyValue: (value: string) => void;
  isSavingPendingKey: boolean;
  handleEnableExternalProvider: (slug: string, apiKey: string) => Promise<void>;
  ttsVoice: string;
  setTtsVoice: (value: string) => void;
  piperVoicePresets: ReadonlyArray<{ id: string; label: string }>;
  piperVoicePresetIds: readonly string[];
  /** True while the panel re-reads `voice_status` after a "Check again" click. */
  isCheckingPiper: boolean;
  handleRecheckPiper: () => Promise<void>;
  piperReady: boolean;
  pendingLocalProviderReady: boolean;
  isSavingProviders: boolean;
  onTtsProviderChange: (next: string) => void;
  persistProviders: (update: { tts_voice?: string }) => Promise<void>;
}

/** Inline API-key / Piper setup modal opened from a provider chip. */
const VoicePanelKeyModal = ({
  t,
  pendingKeySlug,
  setPendingKeySlug,
  pendingKeyValue,
  setPendingKeyValue,
  isSavingPendingKey,
  handleEnableExternalProvider,
  ttsVoice,
  setTtsVoice,
  piperVoicePresets,
  piperVoicePresetIds,
  isCheckingPiper,
  handleRecheckPiper,
  piperReady,
  pendingLocalProviderReady,
  isSavingProviders,
  onTtsProviderChange,
  persistProviders,
}: VoicePanelKeyModalProps) => {
  const [isTestingKey, setIsTestingKey] = useState(false);
  const [keyTestResult, setKeyTestResult] = useState<{ ok: boolean; detail: string } | null>(null);
  // Monotonic id for the in-flight key test. The API-key field stays editable
  // during a test (it is only disabled while *saving*), so without this a
  // result for key A can land next to key B and read as a validation of B —
  // the same "the UI is telling you something untrue about this key" failure
  // this modal is being fixed for. Bumped on every edit and every new test;
  // a response whose id is stale is dropped. Same guard as the LLM routing
  // dialog's `testRequestIdRef`.
  const testRequestIdRef = useRef(0);
  const isPiper = pendingKeySlug === 'piper';

  const close = () => {
    if (isSavingPendingKey) return;
    setPendingKeySlug(null);
    setPendingKeyValue('');
    setKeyTestResult(null);
  };

  return (
    <ModalShell
      titleId="voice-provider-key-title"
      title={
        isPiper
          ? `${t('voice.modal.title')} ${t('voice.providers.chip.piper')}`
          : `${t('voice.modal.title')} ${BUILTIN_VOICE_PROVIDER_META[pendingKeySlug]?.label ?? pendingKeySlug}`
      }
      subtitle={isPiper ? t('voice.modal.piperDesc') : t('voice.modal.desc')}
      onClose={close}
      maxWidthClassName="max-w-md"
      contentClassName="px-5 py-4 space-y-4"
      footer={
        isPiper ? (
          <div className="flex items-center justify-between pt-2">
            <Button
              type="button"
              variant="secondary"
              size="xs"
              onClick={() => {
                setPendingKeySlug(null);
                setKeyTestResult(null);
              }}>
              {t('common.cancel')}
            </Button>
            <Button
              type="button"
              variant="primary"
              size="xs"
              onClick={() => {
                if (!pendingLocalProviderReady) return;
                onTtsProviderChange('piper');
                if (ttsVoice) void persistProviders({ tts_voice: ttsVoice });
                setPendingKeySlug(null);
                setKeyTestResult(null);
              }}
              disabled={!pendingLocalProviderReady || isSavingProviders}>
              {t('voice.modal.enable')}
            </Button>
          </div>
        ) : (
          <div className="flex items-center justify-between pt-2">
            <Button
              type="button"
              variant="secondary"
              size="xs"
              onClick={close}
              disabled={isSavingPendingKey}>
              {t('common.cancel')}
            </Button>

            <div className="flex items-center gap-2">
              <Button
                type="button"
                variant="secondary"
                size="xs"
                disabled={!pendingKeyValue.trim() || isTestingKey || isSavingPendingKey}
                onClick={async () => {
                  if (!pendingKeySlug || !pendingKeyValue.trim()) return;
                  const requestId = testRequestIdRef.current + 1;
                  testRequestIdRef.current = requestId;
                  setIsTestingKey(true);
                  setKeyTestResult(null);
                  try {
                    // Test is a DRY RUN. It must not call
                    // `handleEnableExternalProvider`: that writes the key to
                    // the keychain and activates the provider before it is
                    // known to work, and it clears `pendingKeySlug`, which
                    // unmounts this modal — so the result below would be set
                    // on a dead component and the user would never see it
                    // (#5896). The candidate key goes to the core for
                    // validation only; "Save & Enable" remains the one way to
                    // commit it.
                    const meta = BUILTIN_VOICE_PROVIDER_META[pendingKeySlug];
                    const workload = meta?.capability === 'tts' ? 'tts' : 'stt';
                    const result = await testVoiceProvider(
                      workload as 'stt' | 'tts',
                      pendingKeySlug,
                      true,
                      pendingKeyValue
                    );
                    if (testRequestIdRef.current !== requestId) return;
                    setKeyTestResult(result);
                  } catch (err) {
                    if (testRequestIdRef.current !== requestId) return;
                    setKeyTestResult({
                      ok: false,
                      detail: err instanceof Error ? err.message : 'Test failed',
                    });
                  } finally {
                    // Unconditional: the Test button is disabled while
                    // `isTestingKey`, so there is only ever one test in
                    // flight and this cannot strand the button on "Testing…".
                    setIsTestingKey(false);
                  }
                }}>
                {isTestingKey ? t('voice.modal.testing') : t('voice.modal.testKey')}
              </Button>
              <Button
                type="button"
                variant="primary"
                size="xs"
                onClick={() => void handleEnableExternalProvider(pendingKeySlug, pendingKeyValue)}
                disabled={!pendingKeyValue.trim() || isSavingPendingKey}>
                {isSavingPendingKey ? t('common.loading') : t('voice.modal.saveAndEnable')}
              </Button>
            </div>
          </div>
        )
      }>
      <div data-testid="voice-provider-key-modal" className="space-y-4">
        {isPiper ? (
          <>
            <label className="block space-y-1">
              <span className="text-xs font-medium text-content-muted dark:text-content-secondary">
                {t('voice.providers.piperVoice')}
              </span>
              <SettingsSelect
                value={piperVoicePresetIds.some(v => v === ttsVoice) ? ttsVoice : '__custom__'}
                onChange={e => {
                  if (e.target.value !== '__custom__') setTtsVoice(e.target.value);
                }}
                className="w-full">
                {piperVoicePresets.map(v => (
                  <option key={v.id} value={v.id}>
                    {v.label}
                  </option>
                ))}
                <option value="__custom__">{t('voice.providers.customVoiceOption')}</option>
              </SettingsSelect>
            </label>

            {/* OpenHuman does not download Piper or its voices — the user
                installs them and this only reports whether they resolve. */}
            <p
              data-testid="voice-piper-self-install-hint"
              className="text-xs text-content-muted dark:text-content-secondary">
              {t('voice.providers.piperSelfInstallHint')}
            </p>

            <div className="flex items-center gap-2">
              <Button
                type="button"
                variant="secondary"
                size="xs"
                data-testid="voice-piper-recheck"
                onClick={() => void handleRecheckPiper()}
                disabled={isCheckingPiper}>
                {t('voice.providers.piperRecheck')}
              </Button>
              <span
                data-testid="voice-piper-status"
                className={`text-[11px] ${
                  piperReady ? 'text-sage-600 dark:text-sage-300' : 'text-content-muted'
                }`}>
                {piperReady ? t('voice.providers.piperFound') : t('voice.providers.piperNotFound')}
              </span>
            </div>
          </>
        ) : (
          <>
            <label className="block space-y-1">
              <span className="text-xs font-medium text-content-muted dark:text-content-secondary">
                {t('voice.providers.chip.apiKeyLabel')}
              </span>
              <SettingsTextField
                id="voice-provider-key-input"
                type="password"
                autoComplete="off"
                autoCorrect="off"
                spellCheck={false}
                data-form-type="other"
                data-lpignore="true"
                value={pendingKeyValue}
                onChange={e => {
                  setPendingKeyValue(e.target.value);
                  setKeyTestResult(null);
                  // Any edit invalidates a test still in flight for the old
                  // key, so its result cannot arrive and describe this one.
                  testRequestIdRef.current += 1;
                }}
                disabled={isSavingPendingKey}
                placeholder={t('voice.providers.chip.apiKeyPlaceholder')}
                className="w-full"
              />
            </label>

            {keyTestResult && (
              <Alert
                variant={keyTestResult.ok ? 'success' : 'destructive'}
                className="rounded-md px-3 py-2 text-xs">
                {keyTestResult.detail}
              </Alert>
            )}
          </>
        )}
      </div>
    </ModalShell>
  );
};

export default VoicePanelKeyModal;
