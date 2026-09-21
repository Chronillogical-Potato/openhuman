import { useT } from '../../../lib/i18n/I18nContext';
import { MNEMONIC_GENERATE_WORD_COUNT } from '../../../utils/cryptoKeys';
import { Alert } from '../../ui/Alert';
import Button from '../../ui/Button';
import { CheckIcon } from '../../ui/icons';
import { SettingsCheckbox } from '../controls';

export interface RecoveryPhraseGenerateModeProps {
  words: string[];
  revealed: boolean;
  onReveal: () => void;
  copied: boolean;
  onCopy: () => void;
  confirmed: boolean;
  onConfirmedChange: (next: boolean) => void;
}

// Generate-mode body: the newly generated word grid (blurred until revealed),
// the copy-to-clipboard action, and the consent checkbox that gates Save.
const RecoveryPhraseGenerateMode = ({
  words,
  revealed,
  onReveal,
  copied,
  onCopy,
  confirmed,
  onConfirmedChange,
}: RecoveryPhraseGenerateModeProps) => {
  const { t } = useT();

  return (
    <>
      <div className="mb-4 space-y-3">
        <p className="text-sm text-content-secondary leading-relaxed">
          {t('mnemonic.writeDownWords')} {MNEMONIC_GENERATE_WORD_COUNT} {t('mnemonic.wordsInOrder')}
        </p>
        <Alert variant="info" className="border-none">
          <p className="text-xs leading-relaxed">{t('mnemonic.cannotRecover')}</p>
        </Alert>
      </div>

      <div className="relative bg-surface-muted rounded-2xl p-2 border border-line overflow-hidden mb-4">
        <div
          className={`grid grid-cols-3 gap-2 transition-all duration-300 ${!revealed ? 'blur-[8px] pointer-events-none select-none opacity-40 scale-[0.98]' : ''}`}>
          {words.map((word, index) => (
            <div
              key={index}
              className="flex items-center gap-2 bg-surface rounded-lg px-3 py-2 text-sm border border-line">
              <span className="text-content-muted font-mono text-xs w-5 text-right">
                {index + 1}.
              </span>
              <span className="font-mono font-medium">{word}</span>
            </div>
          ))}
        </div>
        {!revealed && (
          <div
            className="absolute inset-0 z-10 flex flex-col items-center justify-center cursor-pointer bg-black/40 hover:bg-black/30 transition-colors"
            onClick={onReveal}>
            <svg
              className="w-7 h-7 text-white mb-2"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={1.5}>
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M3.98 8.223A10.477 10.477 0 001.934 12C3.226 16.338 7.244 19.5 12 19.5c.993 0 1.953-.138 2.863-.395M6.228 6.228A10.45 10.45 0 0112 4.5c4.756 0 8.773 3.162 10.065 7.498a10.523 10.523 0 01-4.293 5.774M6.228 6.228L3 3m3.228 3.228l3.65 3.65m7.894 7.894L21 21m-3.228-3.228l-3.65-3.65m0 0a3 3 0 10-4.243-4.243m4.242 4.242L9.88 9.88"
              />
            </svg>
            <span className="text-[15px] font-semibold text-white tracking-tight">
              Tap to reveal
            </span>
          </div>
        )}
      </div>

      <Button
        type="button"
        variant="secondary"
        size="lg"
        className="w-full bg-white/10 hover:bg-white/15 text-white border-0 mb-3"
        disabled={!revealed}
        onClick={onCopy}>
        {copied ? (
          <>
            <CheckIcon className="w-4 h-4 text-sage-400" />
            <span className="text-sage-400">{t('common.copied')}</span>
          </>
        ) : (
          t('mnemonic.copyToClipboard')
        )}
      </Button>

      <label
        className={`flex items-start gap-3 mb-4 transition-all duration-200 ${
          revealed ? 'cursor-pointer' : 'opacity-50 pointer-events-none'
        }`}>
        <SettingsCheckbox
          id="mnemonic-confirm-checkbox"
          checked={confirmed}
          onCheckedChange={onConfirmedChange}
        />
        <span className={`text-sm ${revealed ? 'text-content' : 'text-content-secondary'}`}>
          {t('mnemonic.consentSaved')}
        </span>
      </label>
    </>
  );
};

export default RecoveryPhraseGenerateMode;
