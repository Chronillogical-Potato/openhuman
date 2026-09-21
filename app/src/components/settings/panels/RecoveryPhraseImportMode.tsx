import { type KeyboardEvent, type MutableRefObject } from 'react';

import { useT } from '../../../lib/i18n/I18nContext';
import { CheckIcon } from '../../ui/icons';
import TextField from '../../ui/TextField';
import { ToggleGroupItem, ToggleGroupRoot } from '../../ui/ToggleGroup';

const BIP39_IMPORT_LENGTHS = [12, 15, 18, 21, 24] as const;

export interface RecoveryPhraseImportModeProps {
  importWords: string[];
  selectedWordCount: number;
  importValid: boolean | null;
  inputRefs: MutableRefObject<(HTMLInputElement | null)[]>;
  onWordCountChange: (count: number) => void;
  onWordChange: (index: number, value: string) => void;
  onWordKeyDown: (index: number, e: KeyboardEvent<HTMLInputElement>) => void;
}

// Import-mode body: word-count selector, the labelled word-slot grid, the
// valid-phrase banner, and the link back to generate mode.
const RecoveryPhraseImportMode = ({
  importWords,
  selectedWordCount,
  importValid,
  inputRefs,
  onWordCountChange,
  onWordChange,
  onWordKeyDown,
}: RecoveryPhraseImportModeProps) => {
  const { t } = useT();

  return (
    <>
      <div className="mb-4">
        <p className="text-sm text-content-secondary leading-relaxed">
          {t('mnemonic.enterPhraseToRestore')}
        </p>
      </div>

      {/*
        Mutually-exclusive choice, so this is a ToggleGroup `type="single"`
        (Radix `role="radiogroup"` / `role="radio"`) rather than the previous
        `role="group"` of `aria-pressed` buttons. `aria-pressed` describes an
        independent on/off toggle; picking one of five word counts is a radio
        set, and a screen reader now announces "3 of 5" instead of five
        unrelated pressed-states. `onValueChange` guards the empty string —
        Radix emits it when the active item is re-selected, and deselecting is
        not a legal state here.
      */}
      <div className="flex items-center gap-2 mb-3">
        <span className="text-xs text-content-muted">{t('mnemonic.words')}:</span>
        <ToggleGroupRoot
          type="single"
          value={String(selectedWordCount)}
          aria-label={t('mnemonic.words')}
          onValueChange={(next: string) => {
            if (!next) return;
            onWordCountChange(Number(next) as (typeof BIP39_IMPORT_LENGTHS)[number]);
          }}
          className="flex items-center gap-2">
          {BIP39_IMPORT_LENGTHS.map(len => (
            <ToggleGroupItem
              key={len}
              value={String(len)}
              size="xs"
              className="rounded-lg data-[state=on]:bg-primary-500/20 data-[state=on]:border-primary-500/40 data-[state=on]:text-primary-600 dark:data-[state=on]:text-primary-300">
              {len}
            </ToggleGroupItem>
          ))}
        </ToggleGroupRoot>
      </div>

      <div className="relative bg-surface-muted rounded-2xl p-2 border border-line overflow-hidden mb-4">
        <div className="grid grid-cols-3 gap-2">
          {importWords.map((word, index) => (
            <div
              key={index}
              className={`flex items-center gap-2 bg-surface rounded-lg px-3 py-0.5 border border-line transition-shadow overflow-hidden ring-1 ring-transparent ${
                importValid === false && word.trim().length > 0
                  ? 'focus-within:ring-coral-500'
                  : importValid === true
                    ? 'focus-within:ring-sage-500'
                    : 'focus-within:ring-primary-500'
              }`}>
              <span className="text-content-muted font-mono text-xs w-5 text-right shrink-0">
                {index + 1}.
              </span>
              <TextField
                aria-label={`Recovery phrase word ${index + 1}`}
                ref={el => {
                  inputRefs.current[index] = el;
                }}
                type="text"
                value={word}
                onChange={e => onWordChange(index, e.target.value)}
                onKeyDown={e => onWordKeyDown(index, e)}
                autoComplete="off"
                spellCheck={false}
                className="flex-1 bg-transparent border-0 focus:ring-0 px-0 shadow-none font-mono font-medium text-sm text-content h-8"
              />
            </div>
          ))}
        </div>
      </div>

      {importValid === true && (
        <div className="flex items-center gap-2 text-sage-400 text-sm mb-3 justify-center">
          <CheckIcon className="w-4 h-4" />
          <span>{t('mnemonic.validPhrase')}</span>
        </div>
      )}
    </>
  );
};

export default RecoveryPhraseImportMode;
