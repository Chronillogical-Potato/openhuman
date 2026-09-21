import { QRCodeSVG } from 'qrcode.react';
import { useCallback, useEffect, useRef, useState } from 'react';

import { balanceNetworkLabel } from '../../../../features/wallet/walletDisplay';
import { useT } from '../../../../lib/i18n/I18nContext';
import type { BalanceInfo } from '../../../../services/walletApi';
import { Alert } from '../../../ui/Alert';
import Button from '../../../ui/Button';
import { WarningIcon } from '../../../ui/icons';
import { ModalShell } from '../../../ui/ModalShell';

interface ReceiveModalProps {
  balance: BalanceInfo;
  onClose: () => void;
}

/**
 * Receive modal — renders the derived address for the selected chain/network as
 * a QR code plus a copyable string. Receiving is read-only: no signing, no RPC.
 * For EVM the same address works across every EVM network.
 */
const ReceiveModal = ({ balance, onClose }: ReceiveModalProps) => {
  const { t } = useT();
  const baseLabel = balanceNetworkLabel(balance);
  const networkLabel = balance.chain === 'tron' ? 'TRX' : baseLabel;
  const [copied, setCopied] = useState(false);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Clear the "Copied" reset timer if the modal unmounts before it fires.
  useEffect(
    () => () => {
      if (copyTimerRef.current !== null) clearTimeout(copyTimerRef.current);
    },
    []
  );

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(balance.address);
      setCopied(true);
      if (copyTimerRef.current !== null) clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard unavailable; ignore.
    }
  }, [balance.address]);

  return (
    <ModalShell
      onClose={onClose}
      titleId="wallet-receive-title"
      title={t('walletBalances.receive')}
      subtitle={networkLabel}>
      <div className="flex flex-col items-center gap-4">
        <p className="text-xs text-content-muted text-center leading-relaxed">
          {t('walletReceive.scanHint')}
        </p>
        <div className="rounded-xl bg-surface p-3 border border-line" data-testid="receive-qr">
          <QRCodeSVG
            value={balance.address}
            size={180}
            level="M"
            bgColor="#ffffff"
            fgColor="#1c1917"
          />
        </div>
        <div className="w-full -mb-2">
          <span className="block text-[11px] font-medium text-content-muted mb-1 text-left">
            {t('walletReceive.addressLabel').replace('{network}', baseLabel)}
          </span>
          <div className="w-full rounded-xl border border-line bg-surface-muted px-3 py-2.5">
            <span
              className="block font-mono text-[11px] sm:text-xs text-content break-all text-center"
              data-testid="receive-address">
              {balance.address}
            </span>
          </div>
          <div className="mt-2 flex justify-center">
            <Button
              variant="tertiary"
              size="sm"
              onClick={() => void handleCopy()}
              className="text-primary-600 dark:text-primary-400 hover:text-primary-700 dark:hover:text-primary-300 gap-1.5 font-medium">
              {copied ? (
                <svg
                  className="w-4 h-4"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                </svg>
              ) : (
                <svg
                  className="w-4 h-4"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={2}>
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                  />
                </svg>
              )}
              {copied ? t('common.copied') : t('walletBalances.copyAddress')}
            </Button>
          </div>
        </div>

        <Alert variant="info" className="border-none items-start">
          <WarningIcon className="w-4 h-4 shrink-0 mt-0.5" />
          <p className="text-xs leading-relaxed">
            {t('walletReceive.onlyChainWarning').replace('{network}', baseLabel)}
          </p>
        </Alert>
      </div>
    </ModalShell>
  );
};

export default ReceiveModal;
