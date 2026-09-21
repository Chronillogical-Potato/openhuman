import { Dialog as DialogPrimitive } from 'radix-ui';
import { useCallback, useMemo, useState } from 'react';

import {
  formatDisplayBalance,
  fromSmallestUnit,
  toSmallestUnit,
} from '../../../../features/wallet/walletDisplay';
import { useT } from '../../../../lib/i18n/I18nContext';
import {
  type BalanceInfo,
  executePrepared,
  type ExecutionResult,
  type PreparedTransaction,
  prepareTransfer,
} from '../../../../services/walletApi';
import { Alert, AlertDescription } from '../../../ui/Alert';
import Button from '../../../ui/Button';
import { CheckIcon, Spinner } from '../../../ui/icons';
import { InputGroupAddon, InputGroupInput, InputGroupRoot } from '../../../ui/InputGroup';
import Label from '../../../ui/Label';
import TextField from '../../../ui/TextField';
import { ChainIcon } from '../WalletBalancesPanel';

interface SendCryptoModalProps {
  balance: BalanceInfo;
  onClose: () => void;
  // Called after a successful broadcast so the panel can refresh balances.
  onSuccess: () => void;
}

type Step = 'form' | 'review' | 'sending' | 'done';

// Truncate a hash/address to `0x1234…abcd` for compact display.
function truncate(value: string): string {
  if (value.length <= 14) return value;
  return `${value.slice(0, 8)}…${value.slice(-6)}`;
}

/*
 * Send modal — drives the wallet's prepare → confirm → execute flow for the
 * native asset of the selected balance row. `prepareTransfer` builds a quote
 * (with the simulated fee) that the user reviews before `executePrepared`
 * signs locally and broadcasts. Native asset only; token sends are a follow-up.
 */
const SendCryptoModal = ({ balance, onClose, onSuccess }: SendCryptoModalProps) => {
  const { t } = useT();

  const [step, setStep] = useState<Step>('form');
  const [recipient, setRecipient] = useState('');
  const [amount, setAmount] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [prepared, setPrepared] = useState<PreparedTransaction | null>(null);
  const [result, setResult] = useState<ExecutionResult | null>(null);

  const feeFormatted = useMemo(() => {
    if (!prepared) return null;
    return fromSmallestUnit(prepared.estimatedFeeRaw, balance.decimals);
  }, [prepared, balance.decimals]);

  const handleReview = useCallback(async () => {
    setError(null);
    let amountRaw: string;
    try {
      amountRaw = toSmallestUnit(amount, balance.decimals);
    } catch {
      // toSmallestUnit throws dev-facing messages; surface a translated one.
      setError(t('walletSend.invalidAmount'));
      return;
    }
    if (amountRaw === '0') {
      setError(t('walletSend.invalidAmount'));
      return;
    }
    if (recipient.trim() === '') {
      setError(t('walletSend.recipientRequired'));
      return;
    }
    setBusy(true);
    try {
      const quote = await prepareTransfer({
        chain: balance.chain,
        toAddress: recipient.trim(),
        amountRaw,
        evmNetwork: balance.evmNetwork,
      });
      setPrepared(quote);
      setStep('review');
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.debug('[walletSend] prepare failed:', message);
      setError(message || t('walletSend.genericError'));
    } finally {
      setBusy(false);
    }
  }, [amount, recipient, balance, t]);

  const handleConfirm = useCallback(async () => {
    if (!prepared) return;
    setError(null);
    setBusy(true);
    setStep('sending');
    try {
      const executed = await executePrepared(prepared.quoteId);
      setResult(executed);
      setStep('done');
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.debug('[walletSend] execute failed:', message);
      setError(message || t('walletSend.genericError'));
      setStep('review');
    } finally {
      setBusy(false);
    }
  }, [prepared, t]);

  const handleDone = useCallback(() => {
    onSuccess();
    onClose();
  }, [onSuccess, onClose]);

  return (
    <DialogPrimitive.Root
      open={true}
      onOpenChange={next => {
        if (!next) onClose();
      }}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm duration-200 animate-in fade-in data-[state=closed]:animate-out data-[state=closed]:fade-out" />
        <DialogPrimitive.Content
          aria-describedby={undefined}
          className="fixed left-1/2 top-1/2 z-50 w-full max-w-[400px] -translate-x-1/2 -translate-y-1/2 rounded-2xl bg-surface p-0 shadow-2xl duration-200 animate-in fade-in zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out data-[state=closed]:zoom-out-95">
          <div className="flex items-center justify-between border-b border-line-subtle px-4 py-4">
            <div className="w-8" />
            <DialogPrimitive.Title className="text-sm font-semibold text-content m-0 p-0">
              Send
            </DialogPrimitive.Title>
            <button
              onClick={onClose}
              className="flex h-8 w-8 items-center justify-center rounded-full text-content-muted hover:bg-surface-hover hover:text-content transition-colors"
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

          <div className="flex flex-col items-center p-5 pt-6 gap-6">
            <div className="flex flex-col items-center gap-3">
              <div className="scale-150 transform mb-2">
                <ChainIcon chain={balance.chain} evmNetwork={balance.evmNetwork} />
              </div>
              <span className="text-lg font-bold text-content">{balance.assetSymbol}</span>
            </div>

            <div className="w-full">
              {error && (
                <Alert variant="destructive" className="border-0 bg-destructive/10 p-4 mb-4">
                  <AlertDescription className="text-sm leading-relaxed text-destructive opacity-100">
                    {error}
                  </AlertDescription>
                </Alert>
              )}

              {step === 'form' && (
                <div className="flex flex-col gap-4">
                  <div className="flex flex-col gap-1.5">
                    <Label
                      htmlFor="send-recipient-input"
                      className="text-content-muted font-normal ml-1">
                      To
                    </Label>
                    <TextField
                      id="send-recipient-input"
                      type="text"
                      value={recipient}
                      onChange={e => setRecipient(e.target.value)}
                      placeholder="Enter or paste an address"
                      spellCheck={false}
                      autoComplete="off"
                      className="font-mono"
                      data-testid="send-recipient"
                    />
                  </div>

                  <div className="flex flex-col gap-1.5">
                    <div className="flex items-center justify-between ml-1 mb-0.5">
                      <Label htmlFor="send-amount-input" className="text-content-muted font-normal">
                        Amount
                      </Label>
                      <div className="flex items-center gap-2 text-xs">
                        <span className="text-content-muted">
                          {formatDisplayBalance(balance.formatted)} {balance.assetSymbol} available
                        </span>
                        <button
                          type="button"
                          onClick={() => setAmount(balance.formatted)}
                          className="text-primary-500 font-medium hover:text-primary-600 transition-colors">
                          Max
                        </button>
                      </div>
                    </div>
                    <InputGroupRoot>
                      <InputGroupInput
                        id="send-amount-input"
                        type="text"
                        inputMode="decimal"
                        value={amount}
                        onChange={e => setAmount(e.target.value)}
                        placeholder="0.0"
                        className="font-mono"
                        data-testid="send-amount"
                      />
                      <InputGroupAddon className="font-medium">
                        {balance.assetSymbol}
                      </InputGroupAddon>
                    </InputGroupRoot>
                  </div>

                  <Button
                    type="button"
                    onClick={() => void handleReview()}
                    disabled={busy}
                    className="w-full mt-2"
                    data-testid="send-review">
                    {busy ? t('walletSend.preparing') : t('walletSend.review')}
                  </Button>
                </div>
              )}

              {step === 'review' && prepared && (
                <div className="flex flex-col gap-4">
                  <p className="text-xs text-content-muted text-center leading-relaxed">
                    {t('walletSend.confirmHint')}
                  </p>
                  <dl className="flex flex-col gap-3 text-[13px]">
                    <div className="flex items-center justify-between">
                      <dt className="text-content-muted">{t('walletSend.amount')}</dt>
                      <dd className="font-mono font-medium text-content">
                        {formatDisplayBalance(prepared.amountFormatted)} {prepared.assetSymbol}
                      </dd>
                    </div>
                    <div className="flex items-center justify-between">
                      <dt className="text-content-muted">{t('walletSend.recipient')}</dt>
                      <dd className="font-mono text-content">{truncate(prepared.toAddress)}</dd>
                    </div>
                    <div className="flex items-center justify-between">
                      <dt className="text-content-muted">{t('walletSend.estimatedFee')}</dt>
                      <dd className="font-mono text-content" data-testid="send-fee">
                        {feeFormatted ? formatDisplayBalance(feeFormatted) : '0'}{' '}
                        {balance.assetSymbol}
                      </dd>
                    </div>
                  </dl>
                  {prepared.notes.length > 0 && (
                    <ul className="list-disc pl-4 text-xs text-content-muted space-y-0.5">
                      {prepared.notes.map((note, i) => (
                        <li key={i}>{note}</li>
                      ))}
                    </ul>
                  )}
                  <div className="flex gap-2 mt-2">
                    <Button
                      type="button"
                      variant="secondary"
                      onClick={() => {
                        setStep('form');
                        setPrepared(null);
                      }}
                      disabled={busy}
                      className="flex-1">
                      {t('common.back')}
                    </Button>
                    <Button
                      type="button"
                      onClick={() => void handleConfirm()}
                      disabled={busy}
                      className="flex-1"
                      data-testid="send-confirm">
                      {t('walletSend.confirmSend')}
                    </Button>
                  </div>
                </div>
              )}

              {step === 'sending' && (
                <div className="flex flex-col items-center gap-3 py-8 text-content-muted">
                  <Spinner className="w-6 h-6" />
                  <span className="text-sm">{t('walletSend.sending')}</span>
                </div>
              )}

              {step === 'done' && result && (
                <div className="flex flex-col items-center gap-4 py-2 text-center">
                  <div className="w-14 h-14 rounded-full bg-sage-100 dark:bg-sage-500/15 flex items-center justify-center shadow-sm">
                    <CheckIcon className="w-7 h-7 text-sage-600 dark:text-sage-400" />
                  </div>
                  <p className="text-[15px] font-medium text-content">{t('walletSend.sent')}</p>
                  <div className="w-full rounded-xl border border-line bg-surface-muted px-4 py-3 text-left flex flex-col gap-1">
                    <span className="text-[11px] text-content-muted uppercase tracking-wider font-medium">
                      {t('walletSend.txHash')}
                    </span>
                    <span
                      className="font-mono text-xs text-content-secondary break-all"
                      data-testid="send-tx-hash">
                      {result.transactionHash}
                    </span>
                  </div>
                  {result.explorerUrl && (
                    <a
                      href={result.explorerUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-xs font-medium text-primary-600 dark:text-primary-400 hover:text-primary-700 dark:hover:text-primary-300">
                      {t('walletSend.viewExplorer')}
                    </a>
                  )}
                  <Button type="button" onClick={handleDone} className="mt-2 w-full">
                    {t('walletSend.done')}
                  </Button>
                </div>
              )}
            </div>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
};

export default SendCryptoModal;
