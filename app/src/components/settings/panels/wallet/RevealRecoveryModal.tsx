import { Dialog as DialogPrimitive } from 'radix-ui';
import { useCallback, useEffect, useState } from 'react';

import Button from '../../../ui/Button';
import { CheckIcon } from '../../../ui/icons';

interface RevealRecoveryModalProps {
  open: boolean;
  onClose: () => void;
  mnemonic: string;
}

export default function RevealRecoveryModal({ open, onClose, mnemonic }: RevealRecoveryModalProps) {
  const [revealed, setRevealed] = useState(false);
  const [copied, setCopied] = useState(false);

  // Clear sensitive data on unmount/close
  useEffect(() => {
    if (!open) {
      setTimeout(() => {
        setRevealed(false);
      }, 300); // delay cleanup slightly for exit animation
    }
  }, [open]);

  const handleCopy = useCallback(async () => {
    if (!mnemonic) return;
    try {
      await navigator.clipboard.writeText(mnemonic);
      setCopied(true);
      setTimeout(() => setCopied(false), 3000);
    } catch {
      const textarea = document.createElement('textarea');
      textarea.value = mnemonic;
      textarea.style.position = 'fixed';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      const ok = document.execCommand('copy');
      document.body.removeChild(textarea);
      if (ok) {
        setCopied(true);
        setTimeout(() => setCopied(false), 3000);
      }
    }
  }, [mnemonic]);

  const words = mnemonic ? mnemonic.split(' ') : [];

  return (
    <DialogPrimitive.Root
      open={open}
      onOpenChange={next => {
        if (!next) onClose();
      }}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm duration-200 animate-in fade-in data-[state=closed]:animate-out data-[state=closed]:fade-out" />
        <DialogPrimitive.Content
          aria-describedby={undefined}
          className="fixed left-1/2 top-1/2 z-50 w-full max-w-[500px] -translate-x-1/2 -translate-y-1/2 rounded-3xl bg-surface p-0 shadow-2xl duration-200 animate-in fade-in zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out data-[state=closed]:zoom-out-95 flex flex-col max-h-[85vh] border border-white/5">
          <div className="relative flex items-center justify-center px-5 py-4 shrink-0">
            <DialogPrimitive.Title className="text-sm font-semibold text-content m-0 p-0">
              Save Secret Recovery Phrase
            </DialogPrimitive.Title>
            <button
              onClick={onClose}
              className="absolute right-3 flex h-8 w-8 items-center justify-center rounded-full text-content-muted hover:bg-surface-hover hover:text-content transition-colors"
              aria-label="Close">
              <svg
                className="h-5 w-5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          <div className="px-5 pb-5 overflow-y-auto">
            <p className="text-sm text-content-muted leading-relaxed mb-6">
              This phrase is the key to your wallet. Store it somewhere private and in the correct
              order. Anyone with access to it can control your wallet.
            </p>

            {mnemonic ? (
              <div className="relative bg-surface-muted rounded-2xl p-2 border border-line overflow-hidden">
                <div
                  className={`grid grid-cols-3 gap-2 transition-all duration-300 ${!revealed ? 'blur-[8px] pointer-events-none select-none opacity-40 scale-[0.98]' : ''}`}
                  onClick={() => revealed && setRevealed(false)}>
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
                  <button
                    type="button"
                    aria-label="Reveal recovery phrase"
                    className="absolute inset-0 z-10 flex flex-col items-center justify-center cursor-pointer bg-black/40 hover:bg-black/30 transition-colors"
                    onClick={() => setRevealed(true)}>
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
                    <span className="text-[15px] font-semibold text-white mb-1 tracking-tight">
                      Tap to reveal
                    </span>
                    <span className="text-[12px] font-medium text-white/80">
                      Make sure no one is watching your screen.
                    </span>
                  </button>
                )}
              </div>
            ) : null}
          </div>

          <div className="px-5 pb-5 mt-2">
            <Button
              type="button"
              variant="secondary"
              size="lg"
              className="w-full"
              disabled={!revealed || !mnemonic}
              onClick={handleCopy}>
              {copied ? (
                <>
                  <CheckIcon className="w-4 h-4 text-sage-400" />
                  <span className="text-sage-400">Copied!</span>
                </>
              ) : (
                'Copy to clipboard'
              )}
            </Button>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
