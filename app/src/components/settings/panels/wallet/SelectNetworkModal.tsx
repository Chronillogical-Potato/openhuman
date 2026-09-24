import { Dialog as DialogPrimitive } from 'radix-ui';

import { cn } from '../../../../lib/cn';
import type { NetworkFilterId } from '../WalletBalancesPanel';

interface SelectNetworkModalProps {
  open: boolean;
  onClose: () => void;
  selectedNetwork: NetworkFilterId;
  onSelect: (network: NetworkFilterId) => void;
  networkFilters: readonly { id: NetworkFilterId; label: string }[];
  chainIcons: Record<string, string>;
}

export default function SelectNetworkModal({
  open,
  onClose,
  selectedNetwork,
  onSelect,
  networkFilters,
  chainIcons,
}: SelectNetworkModalProps) {
  return (
    <DialogPrimitive.Root open={open} onOpenChange={(next: boolean) => !next && onClose()}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm duration-200 animate-in fade-in data-[state=closed]:animate-out data-[state=closed]:fade-out" />
        <DialogPrimitive.Content
          aria-describedby={undefined}
          className="fixed left-1/2 top-1/2 z-50 w-full max-w-[340px] -translate-x-1/2 -translate-y-1/2 rounded-2xl bg-surface p-0 shadow-2xl duration-200 animate-in fade-in zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out data-[state=closed]:zoom-out-95">
          <div className="flex items-center justify-between border-b border-line-subtle px-4 py-4">
            <div className="w-8" />
            <DialogPrimitive.Title className="text-sm font-semibold text-content m-0 p-0">
              Select network
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
            {networkFilters.map(filter => {
              const isSelected = selectedNetwork === filter.id;
              const iconSrc = chainIcons[filter.id];

              return (
                <button
                  type="button"
                  key={filter.id}
                  aria-pressed={isSelected}
                  onClick={() => {
                    onSelect(filter.id);
                    onClose();
                  }}
                  className={cn(
                    'flex w-full cursor-pointer items-center justify-between px-4 py-3 transition-colors hover:bg-surface-hover/50',
                    isSelected && 'bg-surface-hover/30'
                  )}>
                  <div className="flex items-center gap-3">
                    {iconSrc ? (
                      <img src={iconSrc} alt="" className="h-6 w-6 rounded-full object-contain" />
                    ) : (
                      <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-surface-muted text-content-muted">
                        <svg
                          className="h-4 w-4"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor">
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={2}
                            d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9"
                          />
                        </svg>
                      </div>
                    )}
                    <span className={cn('text-sm font-medium', 'text-content')}>
                      {filter.label}
                    </span>
                  </div>

                  {isSelected && (
                    <svg
                      className="h-5 w-5 text-primary-500"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                      strokeWidth={2.5}>
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        d="M4.5 12.75l6 6 9-13.5"
                      />
                    </svg>
                  )}
                </button>
              );
            })}
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
