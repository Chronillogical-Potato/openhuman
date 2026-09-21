import { useCallback, useEffect, useRef, useState } from 'react';
import { useSelector } from 'react-redux';

import baseIcon from '../../../assets/icons/base.svg';
import bitcoinIcon from '../../../assets/icons/bitcoin.svg';
import bscIcon from '../../../assets/icons/bsc.svg';
import ethereumIcon from '../../../assets/icons/ethereum.svg';
import networkBaseIcon from '../../../assets/icons/network_base.svg';
import networkBitcoinIcon from '../../../assets/icons/network_bitcoin.svg';
import networkBscIcon from '../../../assets/icons/network_bsc.svg';
import networkEthIcon from '../../../assets/icons/network_eth.svg';
import networkSolanaIcon from '../../../assets/icons/network_solana.svg';
import networkTronIcon from '../../../assets/icons/network_tron.svg';
import solanaIcon from '../../../assets/icons/solana.svg';
import tokenBnbIcon from '../../../assets/icons/tokens/bnb.svg';
import tokenBtcIcon from '../../../assets/icons/tokens/btc.svg';
import tokenEthIcon from '../../../assets/icons/tokens/eth.svg';
import tokenSolIcon from '../../../assets/icons/tokens/sol.svg';
import tokenTronIcon from '../../../assets/icons/tokens/tron.svg';
import tronIcon from '../../../assets/icons/tron.svg';
import {
  balanceKey,
  balanceNetworkLabel,
  formatDisplayBalance,
} from '../../../features/wallet/walletDisplay';
// ---------------------------------------------------------------------------
// WalletBalancesPanel — main panel
// ---------------------------------------------------------------------------

import { useUser } from '../../../hooks/useUser';
import { cn } from '../../../lib/cn';
import { useT } from '../../../lib/i18n/I18nContext';
import {
  type BalanceInfo,
  type EvmNetwork,
  fetchWalletBalances,
  fetchWalletStatus,
  type WalletChain,
} from '../../../services/walletApi';
import { type RootState } from '../../../store';
import { type DataTableColumn } from '../../ui';
import Button from '../../ui/Button';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../../ui/Table';
import { SettingsEmptyState } from '../controls';
import { useSettingsNavigation } from '../hooks/useSettingsNavigation';
import SettingsPanel from '../layout/SettingsPanel';
import ManageTokensModal from './wallet/ManageTokensModal';
import ReceiveModal from './wallet/ReceiveModal';
import SelectNetworkModal from './wallet/SelectNetworkModal';
import SendCryptoModal from './wallet/SendCryptoModal';

export const CHAIN_ICONS: Record<string, string> = {
  ethereum_mainnet: ethereumIcon,
  base_mainnet: baseIcon,
  bsc_mainnet: bscIcon,
  btc: bitcoinIcon,
  solana: solanaIcon,
  tron: tronIcon,
};

export const NETWORK_MODAL_ICONS: Record<string, string> = {
  ethereum_mainnet: networkEthIcon,
  base_mainnet: networkBaseIcon,
  bsc_mainnet: networkBscIcon,
  btc: networkBitcoinIcon,
  solana: networkSolanaIcon,
  tron: networkTronIcon,
};

export function ChainIcon({
  chain,
  evmNetwork,
}: {
  chain: WalletChain;
  evmNetwork?: EvmNetwork;
}) {
  const key = evmNetwork || chain;
  const src = CHAIN_ICONS[key];
  if (!src) return null;
  return <img src={src} alt="" aria-hidden className="w-10 h-10 shrink-0 object-contain" />;
}

export const TOKEN_ICONS: Record<string, string> = {
  ETH: tokenEthIcon,
  BNB: tokenBnbIcon,
  BTC: tokenBtcIcon,
  SOL: tokenSolIcon,
  TRX: tokenTronIcon,
};

// Chain badge colours
const PLACEHOLDER_ROWS: Array<{
  chain: WalletChain;
  evmNetwork?: EvmNetwork;
  assetSymbol: string;
}> = [
  { chain: 'evm', evmNetwork: 'ethereum_mainnet', assetSymbol: 'ETH' },
  { chain: 'evm', evmNetwork: 'base_mainnet', assetSymbol: 'ETH' },
  { chain: 'evm', evmNetwork: 'bsc_mainnet', assetSymbol: 'BNB' },
  { chain: 'btc', assetSymbol: 'BTC' },
  { chain: 'solana', assetSymbol: 'SOL' },
  { chain: 'tron', assetSymbol: 'TRX' },
];

export const NETWORK_FILTERS = [
  { id: 'all', label: 'All networks' },
  { id: 'ethereum_mainnet', label: 'Ethereum' },
  { id: 'base_mainnet', label: 'Base' },
  { id: 'bsc_mainnet', label: 'BNB Smart Chain' },
  { id: 'btc', label: 'Bitcoin' },
  { id: 'solana', label: 'Solana' },
  { id: 'tron', label: 'TRON' },
] as const;

export type NetworkFilterId = (typeof NETWORK_FILTERS)[number]['id'];

// Shorten address for display
function truncateAddress(address: string): string {
  if (address.length <= 18) return address;
  return `${address.slice(0, 8)}…${address.slice(-8)}`;
}

/**
 * Shared column set. Both the real balances and the pre-setup placeholders
 * render through it, so the two states line up column-for-column instead of
 * being two differently-shaped lists.
 *
 * `cell` is omitted throughout: every row here is custom-rendered (a row owns
 * its own copy button, its own clipboard state and its own action pair), and
 * the columns exist to define the header and the alignment.
 */
const COLUMNS_FOR = <T,>(t: (key: string) => string): DataTableColumn<T>[] => [
  { id: 'network', header: 'Token' },
  { id: 'address', header: t('walletBalances.colAddress') },
  {
    id: 'balance',
    header: t('walletBalances.colBalance'),
    align: 'right',
    className: 'pr-12 lg:pr-24',
  },
  { id: 'actions', header: t('walletBalances.colActions') },
];

// ---------------------------------------------------------------------------
// BalanceRow — a single chain/network entry with Send / Receive actions
// ---------------------------------------------------------------------------

interface BalanceRowProps {
  balance: BalanceInfo;
  onSend: (balance: BalanceInfo) => void;
  onReceive: (balance: BalanceInfo) => void;
}

const BalanceRow = ({ balance, onSend, onReceive }: BalanceRowProps) => {
  const { t } = useT();
  const [copied, setCopied] = useState(false);
  // Tracks the most recent "Copied" timer so rapid re-clicks reset the 2s
  // window rather than stacking independent setTimeouts (the older one would
  // otherwise flip `copied` back to false while the newest click still wants
  // to show the checkmark).
  const copyResetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(
    () => () => {
      if (copyResetTimerRef.current !== null) {
        clearTimeout(copyResetTimerRef.current);
        copyResetTimerRef.current = null;
      }
    },
    []
  );

  const handleCopyAddress = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(balance.address);
      setCopied(true);
      if (copyResetTimerRef.current !== null) {
        clearTimeout(copyResetTimerRef.current);
      }
      copyResetTimerRef.current = setTimeout(() => {
        setCopied(false);
        copyResetTimerRef.current = null;
      }, 2000);
    } catch {
      // Clipboard unavailable (no permissions); silently skip.
    }
  }, [balance.address]);

  const networkLabel = balanceNetworkLabel(balance);

  return (
    <TableRow
      data-testid={`wallet-row-${balanceKey(balance)}`}
      className="hover:bg-surface-muted group">
      <TableCell className="whitespace-nowrap px-4">
        <div className="flex items-center gap-3">
          <ChainIcon chain={balance.chain} evmNetwork={balance.evmNetwork} />
          <div className="flex flex-col">
            <span className="text-sm font-bold text-content font-mono">{balance.assetSymbol}</span>
            <span className="text-xs text-content-muted">{networkLabel}</span>
          </div>
          {balance.providerStatus !== 'ready' && (
            <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400">
              {t('walletBalances.providerMissing')}
            </span>
          )}
        </div>
      </TableCell>
      <TableCell>
        {/* Address + copy button */}
        <div className="flex items-center justify-start gap-1.5 min-w-0">
          <span className="font-mono text-[12px] text-content-muted truncate">
            {truncateAddress(balance.address)}
          </span>
          <Button
            type="button"
            iconOnly
            variant="tertiary"
            size="sm"
            onClick={() => void handleCopyAddress()}
            aria-label={t('walletBalances.copyAddress')}
            className="shrink-0 text-content-faint hover:text-content-secondary dark:hover:text-content-secondary">
            {copied ? (
              <svg
                className="w-3.5 h-3.5 text-sage-500"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
            ) : (
              <svg
                className="w-3.5 h-3.5"
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
          </Button>
        </div>
      </TableCell>
      <TableCell className="whitespace-nowrap text-right pr-12 lg:pr-24">
        <div className="flex items-center justify-end gap-1.5">
          <span
            title={t('walletBalances.rawBalance').replace('{raw}', balance.raw)}
            className="text-sm font-medium text-content font-mono">
            {formatDisplayBalance(balance.formatted)}
          </span>
          {TOKEN_ICONS[balance.assetSymbol] ? (
            <img
              src={TOKEN_ICONS[balance.assetSymbol]}
              alt={balance.assetSymbol}
              title={balance.assetSymbol}
              className="w-4 h-4 shrink-0 object-contain opacity-40 dark:opacity-40 dark:invert"
            />
          ) : (
            <span className="text-xs text-content-muted">{balance.assetSymbol}</span>
          )}
        </div>
      </TableCell>
      <TableCell className="w-px whitespace-nowrap text-left">
        <div className="flex justify-start gap-4 pr-2">
          <button
            type="button"
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-surface dark:bg-white/5 border border-line hover:bg-surface-hover dark:hover:bg-white/10 group focus-visible:outline-none transition-all duration-200 hover:scale-105 active:scale-95"
            onClick={() => onSend(balance)}
            data-testid={`wallet-send-${balanceKey(balance)}`}
            aria-label={t('walletBalances.send')}>
            <svg
              className="h-3.5 w-3.5 text-content-secondary group-hover:text-content transition-colors"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2.5}>
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M4.5 10.5L12 3m0 0l7.5 7.5M12 3v18"
              />
            </svg>
            <span className="text-xs font-medium text-content-secondary group-hover:text-content transition-colors">
              {t('walletBalances.send')}
            </span>
          </button>
          <button
            type="button"
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-surface dark:bg-white/5 border border-line hover:bg-surface-hover dark:hover:bg-white/10 group focus-visible:outline-none transition-all duration-200 hover:scale-105 active:scale-95"
            onClick={() => onReceive(balance)}
            data-testid={`wallet-receive-${balanceKey(balance)}`}
            aria-label={t('walletBalances.receive')}>
            <svg
              className="h-3.5 w-3.5 text-content-secondary group-hover:text-content transition-colors"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2.5}>
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M19.5 13.5L12 21m0 0l-7.5-7.5M12 21V3"
              />
            </svg>
            <span className="text-xs font-medium text-content-secondary group-hover:text-content transition-colors">
              {t('walletBalances.receive')}
            </span>
          </button>
        </div>
      </TableCell>
    </TableRow>
  );
};

// ---------------------------------------------------------------------------
// ChainPlaceholderRow — shown per chain before the wallet is configured. There
// is no derived address or balance yet, so we render a muted "not set up" row
// to convey the wallet layout without fabricating data.
// ---------------------------------------------------------------------------

const ChainPlaceholderRow = ({
  chain,
  evmNetwork,
  assetSymbol,
}: {
  chain: WalletChain;
  evmNetwork?: EvmNetwork;
  assetSymbol: string;
}) => {
  const { t } = useT();

  return (
    <TableRow className="hover:bg-white/5 opacity-70 group">
      <TableCell className="whitespace-nowrap px-4">
        <div className="flex items-center gap-3">
          <ChainIcon chain={chain} evmNetwork={evmNetwork} />
          <div className="flex flex-col">
            <span className="text-sm font-bold text-content-faint font-mono">{assetSymbol}</span>
            <span className="text-xs text-content-faint">
              {balanceNetworkLabel({ chain, evmNetwork })}
            </span>
          </div>
        </div>
      </TableCell>
      <TableCell>
        <span className="font-mono text-[11px] text-content-faint">
          {t('walletBalances.notSetUp')}
        </span>
      </TableCell>
      <TableCell className="whitespace-nowrap text-right pr-12 lg:pr-24">
        <div className="flex items-center justify-end gap-1.5">
          <span className="text-sm font-medium text-content-faint font-mono">—</span>
          {TOKEN_ICONS[assetSymbol] ? (
            <img
              src={TOKEN_ICONS[assetSymbol]}
              alt={assetSymbol}
              title={assetSymbol}
              className="w-4 h-4 shrink-0 object-contain opacity-40 dark:opacity-40 dark:invert"
            />
          ) : (
            <span className="text-xs text-content-faint">{assetSymbol}</span>
          )}
        </div>
      </TableCell>
      <TableCell className="w-px" />
    </TableRow>
  );
};

// ---------------------------------------------------------------------------
// WalletBalancesPanel — main panel
// ---------------------------------------------------------------------------

// Keep balances cached when switching tabs (keyed by user ID to prevent cross-user leakage)
const cachedBalances: Record<string, BalanceInfo[] | null> = {};
const cachedWalletConfigured: Record<string, boolean | null> = {};

const WalletBalancesPanel = () => {
  const { t } = useT();
  const { navigateToSettings } = useSettingsNavigation();
  const { user } = useUser();
  const userId = user?._id || 'anonymous';

  const [balances, setBalances] = useState<BalanceInfo[] | null>(cachedBalances[userId] ?? null);
  const [loading, setLoading] = useState(
    cachedBalances[userId] === undefined || cachedBalances[userId] === null
  );
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isManageModalOpen, setIsManageModalOpen] = useState(false);
  const [isNetworkModalOpen, setIsNetworkModalOpen] = useState(false);
  // null = unknown (not yet loaded); false = wallet has no recovery phrase set
  // up yet, in which case we show a hint + placeholder rows instead of erroring.
  const [walletConfigured, setWalletConfigured] = useState<boolean | null>(
    cachedWalletConfigured[userId] ?? null
  );
  // The balance row a Send / Receive modal is currently open for (null = none).
  const [sendTarget, setSendTarget] = useState<BalanceInfo | null>(null);
  const [receiveTarget, setReceiveTarget] = useState<BalanceInfo | null>(null);

  const [selectedNetwork, setSelectedNetwork] = useState<NetworkFilterId>('all');

  const hiddenTokenKeys = useSelector(
    (state: RootState) => state.walletPreferences?.hiddenTokenKeys || []
  );

  // Request-sequencing guard: a slower earlier request must not overwrite a
  // newer one. `loadBalances` can fire concurrently (mount + Refresh + Retry),
  // so we tag each call with a monotonic id and drop any response whose id no
  // longer matches the latest dispatched call.
  const latestRequestIdRef = useRef(0);

  useEffect(() => {
    setBalances(cachedBalances[userId] ?? null);
    setWalletConfigured(cachedWalletConfigured[userId] ?? null);
    setLoading(cachedBalances[userId] === undefined || cachedBalances[userId] === null);
    setError(null);
    setSendTarget(null);
    setReceiveTarget(null);
  }, [userId]);

  const loadBalances = useCallback(async () => {
    const requestId = ++latestRequestIdRef.current;
    if (!cachedBalances[userId]) {
      setLoading(true);
    } else {
      setIsRefreshing(true);
    }
    setError(null);
    try {
      // Check setup state first: the core errors `wallet_balances` when no
      // recovery phrase is configured. Rather than blocking the panel on that,
      // detect it via the structured `configured` flag and fall through to the
      // hint + placeholder rows.
      const status = await fetchWalletStatus();
      if (requestId !== latestRequestIdRef.current) return;
      if (!status.configured) {
        cachedWalletConfigured[userId] = false;
        cachedBalances[userId] = [];
        setWalletConfigured(false);
        setBalances([]);
        return;
      }
      cachedWalletConfigured[userId] = true;
      setWalletConfigured(true);
      const rows = await fetchWalletBalances();
      if (requestId !== latestRequestIdRef.current) return;
      cachedBalances[userId] = rows;
      setBalances(rows);
    } catch (err) {
      if (requestId !== latestRequestIdRef.current) return;
      const message = err instanceof Error ? err.message : String(err);
      // Log the raw backend phrasing for diagnostics; the UI surfaces a
      // translated, user-facing copy via `walletBalances.errorGeneric`.
      console.debug('[walletBalances] fetch failed:', message);
      setError(message);
    } finally {
      if (requestId === latestRequestIdRef.current) {
        setLoading(false);
        setIsRefreshing(false);
      }
    }
  }, [userId]);

  useEffect(() => {
    void loadBalances();
  }, [loadBalances]);

  const selectedNetworkLabel =
    NETWORK_FILTERS.find(f => f.id === selectedNetwork)?.label ?? 'All networks';

  const filterRows = <T extends { chain: WalletChain; evmNetwork?: EvmNetwork; assetSymbol: string }>(rows: T[]) => {
    return rows.filter(row => {
      const networkId = row.chain === 'evm' ? row.evmNetwork : row.chain;
      const bKey = balanceKey(row);
      if (hiddenTokenKeys.includes(bKey)) return false;
      if (selectedNetwork === 'all') return true;
      return networkId === selectedNetwork;
    });
  };

  const renderContent = () => {
    if (loading) {
      return (
        <div className="flex items-center justify-center gap-2 py-10 text-content-muted">
          <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
            <circle
              className="opacity-25"
              cx="12"
              cy="12"
              r="10"
              stroke="currentColor"
              strokeWidth="4"
            />
            <path
              className="opacity-75"
              fill="currentColor"
              d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
            />
          </svg>
          <span className="text-sm">{t('walletBalances.loading')}</span>
        </div>
      );
    }

    if (error) {
      return (
        <div className="px-4 py-4">
          <div
            role="alert"
            className="flex items-start gap-2.5 p-3 mb-4 rounded-xl bg-coral-50 dark:bg-coral-500/10 border border-coral-200 dark:border-coral-500/30">
            <svg
              className="w-4 h-4 text-coral-500 shrink-0 mt-0.5"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}>
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M12 9v2m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"
              />
            </svg>
            <p className="text-xs text-coral-700 dark:text-coral-300 leading-relaxed">
              {t('walletBalances.errorGeneric')}
            </p>
          </div>
          <Button
            type="button"
            variant="primary"
            size="md"
            onClick={() => void loadBalances()}
            className="w-full">
            {t('walletBalances.retry')}
          </Button>
        </div>
      );
    }

    // Wallet not set up yet: show a non-blocking hint plus placeholder rows so
    // the wallet layout is visible even before a recovery phrase exists.
    if (walletConfigured === false) {
      return (
        <div>
          <div className="px-4 pt-4 pb-3">
            <div
              role="status"
              className="flex items-start gap-2.5 p-3 rounded-xl bg-destructive/10 border-0">
              <svg
                className="w-4 h-4 text-destructive shrink-0 mt-0.5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}>
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M12 9v2m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"
                />
              </svg>
              <div className="flex-1 min-w-0">
                <p className="text-sm text-destructive leading-relaxed">
                  {t('walletBalances.setupHint')}
                </p>
                <Button
                  type="button"
                  variant="tertiary"
                  onClick={() => navigateToSettings('recovery-phrase')}
                  className="mt-2 text-sm font-medium text-primary-800 dark:text-primary-400 hover:text-primary-900 dark:hover:text-primary-300 px-0 h-auto">
                  {t('walletBalances.setupCta')}
                </Button>
              </div>
            </div>
          </div>
          <Table containerClassName="w-full bg-transparent border border-line rounded-[24px] overflow-hidden">
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                {COLUMNS_FOR<(typeof PLACEHOLDER_ROWS)[number]>(t).map(col => (
                  <TableHead
                    key={col.id}
                    className={cn(
                      col.align === 'right' && 'text-right',
                      'text-content font-medium pb-4 pt-6',
                      col.className
                    )}>
                    <span className="pb-1">{col.header}</span>
                  </TableHead>
                ))}
              </TableRow>
            </TableHeader>
            <TableBody>
              {filterRows(PLACEHOLDER_ROWS).map(row => (
                <ChainPlaceholderRow
                  key={row.evmNetwork || row.chain}
                  chain={row.chain}
                  evmNetwork={row.evmNetwork}
                  assetSymbol={row.assetSymbol}
                />
              ))}
            </TableBody>
          </Table>
        </div>
      );
    }

    if (balances !== null && balances.length === 0) {
      return (
        <div className="px-4 py-8 text-center">
          <div className="w-12 h-12 rounded-full bg-surface-subtle flex items-center justify-center mx-auto mb-3">
            <svg
              className="w-6 h-6 text-content-faint"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={1.5}>
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M21 12a2.25 2.25 0 00-2.25-2.25H15a3 3 0 11-6 0H5.25A2.25 2.25 0 003 12m18 0v6a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 18v-6m18 0V9M3 12V9m18-3a2.25 2.25 0 00-2.25-2.25H5.25A2.25 2.25 0 003 6m18 0V5.25A2.25 2.25 0 0018.75 3H5.25A2.25 2.25 0 003 5.25V6"
              />
            </svg>
          </div>
          <SettingsEmptyState label={t('walletBalances.emptyState')} />
        </div>
      );
    }

    if (balances && balances.length > 0) {
      const visibleRows = filterRows(balances);
      
      if (visibleRows.length === 0) {
        return (
          <div className="px-4 py-8 text-center">
            <SettingsEmptyState label="No balances match your filters" />
          </div>
        );
      }

      return (
        <Table containerClassName="w-full bg-transparent border border-line rounded-[24px] overflow-hidden">
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              {COLUMNS_FOR<BalanceInfo>(t).map(col => (
                <TableHead
                  key={col.id}
                  className={cn(
                    col.align === 'right' && 'text-right',
                    'text-content font-medium pb-4 pt-6',
                    col.className
                  )}>
                  <span className="pb-1">{col.header}</span>
                </TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {visibleRows.map(balance => (
              <BalanceRow
                key={balanceKey(balance)}
                balance={balance}
                onSend={setSendTarget}
                onReceive={setReceiveTarget}
              />
            ))}
          </TableBody>
        </Table>
      );
    }

    return null;
  };

  const rowsToRender = walletConfigured === false ? PLACEHOLDER_ROWS : balances || [];
  return (
    <SettingsPanel
      title={
        <div className="flex items-center gap-2.5">
          <svg
            className="w-5 h-5 text-white"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}>
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M21 12a2.25 2.25 0 00-2.25-2.25H15a3 3 0 11-6 0H5.25A2.25 2.25 0 003 12m18 0v6a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 18v-6m18 0V9M3 12V9m18-3a2.25 2.25 0 00-2.25-2.25H5.25A2.25 2.25 0 003 6m18 0V5.25A2.25 2.25 0 0018.75 3H5.25A2.25 2.25 0 003 5.25V6"
            />
          </svg>
          {t('pages.settings.account.walletBalances')}
        </div>
      }
      description={t('pages.settings.account.walletBalancesDesc')}>
      <div className="w-full max-w-4xl mx-auto py-2">
        <div className="flex items-center justify-between mb-4 px-3">
          <div>
            <button
              type="button"
              onClick={() => setIsNetworkModalOpen(true)}
              className="flex items-center gap-2 px-3 py-1.5 rounded-full border border-line bg-surface hover:bg-surface-hover transition-colors text-sm font-medium text-content outline-hidden focus-visible:ring-2 focus-visible:ring-primary-500/20">
              <svg
                className="w-4 h-4 text-content-muted"
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
              {selectedNetworkLabel}
            </button>
            <SelectNetworkModal
              open={isNetworkModalOpen}
              onClose={() => setIsNetworkModalOpen(false)}
              selectedNetwork={selectedNetwork}
              onSelect={setSelectedNetwork}
              networkFilters={NETWORK_FILTERS}
              chainIcons={NETWORK_MODAL_ICONS}
            />
          </div>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => void loadBalances()}
              disabled={loading || isRefreshing}
              aria-label="Refresh balances"
              className="flex items-center gap-2 px-3 py-1.5 rounded-full border border-line bg-surface hover:bg-surface-hover transition-colors text-sm font-medium text-content outline-hidden focus-visible:ring-2 focus-visible:ring-primary-500/20 disabled:opacity-50">
              <svg
                className={cn(
                  'w-4 h-4 text-content-muted',
                  (loading || isRefreshing) && 'animate-spin'
                )}
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2.5}>
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
                />
              </svg>
              Refresh
            </button>

            <button
              type="button"
              onClick={() => setIsManageModalOpen(true)}
              aria-label="Manage tokens"
              className="flex items-center gap-2 px-3 py-1.5 rounded-full border border-line bg-surface hover:bg-surface-hover transition-colors text-sm font-medium text-content outline-hidden focus-visible:ring-2 focus-visible:ring-primary-500/20">
              <svg
                className="w-4 h-4 text-content-muted"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2.5}>
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
                />
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                />
              </svg>
              Manage tokens
            </button>
          </div>
        </div>
        {renderContent()}
      </div>

      {sendTarget && (
        <SendCryptoModal
          balance={sendTarget}
          onClose={() => setSendTarget(null)}
          onSuccess={() => void loadBalances()}
        />
      )}
      {receiveTarget && (
        <ReceiveModal balance={receiveTarget} onClose={() => setReceiveTarget(null)} />
      )}
      <ManageTokensModal
        open={isManageModalOpen}
        onClose={() => setIsManageModalOpen(false)}
        tokens={rowsToRender}
      />
    </SettingsPanel>
  );
};

export default WalletBalancesPanel;
