import { Dialog as DialogPrimitive } from 'radix-ui';
import { useDispatch, useSelector } from 'react-redux';

import { balanceAssetName, balanceKey, formatDisplayBalance } from '../../../../features/wallet/walletDisplay';
import { type EvmNetwork, type WalletChain } from '../../../../services/walletApi';
import { type RootState } from '../../../../store';
import { toggleTokenHidden } from '../../../../store/walletPreferencesSlice';
import Switch from '../../../ui/Switch';
import { ChainIcon } from '../WalletBalancesPanel';

interface ManageTokensModalProps {
  open: boolean;
  onClose: () => void;
  tokens: Array<{
    chain: WalletChain;
    evmNetwork?: EvmNetwork;
    assetSymbol: string;
    formatted?: string;
  }>;
}

export default function ManageTokensModal({ open, onClose, tokens }: ManageTokensModalProps) {
  const dispatch = useDispatch();
  const hiddenTokenKeys = useSelector(
    (state: RootState) => state.walletPreferences?.hiddenTokenKeys || []
  );

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
          className="fixed left-1/2 top-1/2 z-50 w-full max-w-[400px] -translate-x-1/2 -translate-y-1/2 rounded-2xl bg-surface p-0 shadow-2xl duration-200 animate-in fade-in zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out data-[state=closed]:zoom-out-95">
          <div className="flex items-center justify-between border-b border-line-subtle px-4 py-4">
            <div className="w-8" />
            <DialogPrimitive.Title className="text-sm font-semibold text-content m-0 p-0">
              Manage tokens
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

          <div className="flex flex-col py-2 max-h-[60vh] overflow-y-auto">
            {tokens.map(token => {
              const key = balanceKey(token);
              const isHidden = hiddenTokenKeys.includes(key);
              const isVisible = !isHidden;
              const assetName = balanceAssetName(token.assetSymbol);
              const displayBalance = token.formatted ? formatDisplayBalance(token.formatted) : '0';

              return (
                <label
                  key={key}
                  className="flex cursor-pointer items-center justify-between rounded-lg py-3 px-2 transition-colors hover:bg-surface-hover/50">
                  <div className="flex items-center gap-3">
                    <ChainIcon chain={token.chain} evmNetwork={token.evmNetwork} />
                    <div className="flex flex-col">
                      <span className="text-sm font-medium text-content">{assetName}</span>
                      <span className="text-xs text-content-muted uppercase mt-0.5">
                        {displayBalance} {token.assetSymbol}
                      </span>
                    </div>
                  </div>
                  <Switch
                    id={`toggle-${key}`}
                    checked={isVisible}
                    onCheckedChange={() => dispatch(toggleTokenHidden({ tokenKey: key }))}
                    thumbClassName="bg-white dark:bg-white"
                  />
                </label>
              );
            })}
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
