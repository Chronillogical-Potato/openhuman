// Display helpers for the wallet balances surface: human-readable EVM network
// labels and lossless conversion between human-entered amounts and the chain's
// smallest unit (wei / sat / lamport / sun) using BigInt (no float rounding).
//
// Chain / network proper names (Ethereum, Base, BNB Chain, …) are brand names
// rendered verbatim, so they intentionally bypass i18n.
import type { BalanceInfo, EvmNetwork, WalletChain } from '../../services/walletApi';

/** Full display name per EVM network. */
const EVM_NETWORK_LABEL: Record<EvmNetwork, string> = {
  ethereum_mainnet: 'Ethereum',
  base_mainnet: 'Base',
  arbitrum_one: 'Arbitrum',
  optimism_mainnet: 'Optimism',
  polygon_mainnet: 'Polygon',
  bsc_mainnet: 'BNB Smart Chain',
};

/** Short badge label per EVM network. */
const EVM_NETWORK_BADGE: Record<EvmNetwork, string> = {
  ethereum_mainnet: 'ETH',
  base_mainnet: 'BASE',
  arbitrum_one: 'ARB',
  optimism_mainnet: 'OP',
  polygon_mainnet: 'POL',
  bsc_mainnet: 'BSC',
};

const CHAIN_LABEL: Record<WalletChain, string> = {
  evm: 'EVM',
  btc: 'Bitcoin',
  solana: 'Solana',
  tron: 'TRON',
};

/** Human network/chain label for a balance row (network name for EVM rows). */
export function balanceNetworkLabel(balance: Pick<BalanceInfo, 'chain' | 'evmNetwork'>): string {
  if (balance.chain === 'evm' && balance.evmNetwork) {
    return EVM_NETWORK_LABEL[balance.evmNetwork] ?? 'EVM';
  }
  return CHAIN_LABEL[balance.chain] ?? balance.chain.toUpperCase();
}

/** Short badge text for a balance row. */
export function balanceBadge(balance: Pick<BalanceInfo, 'chain' | 'evmNetwork'>): string {
  if (balance.chain === 'evm' && balance.evmNetwork) {
    return EVM_NETWORK_BADGE[balance.evmNetwork] ?? 'EVM';
  }
  return { evm: 'EVM', btc: 'BTC', solana: 'SOL', tron: 'TRX' }[balance.chain];
}

/** Stable React key for a balance row (chain + network + symbol). */
export function balanceKey(
  balance: Pick<BalanceInfo, 'chain' | 'evmNetwork' | 'assetSymbol'>
): string {
  return `${balance.chain}-${balance.evmNetwork ?? 'native'}-${balance.assetSymbol}`;
}

const ASSET_NAME: Record<string, string> = {
  ETH: 'Ethereum',
  BTC: 'Bitcoin',
  SOL: 'Solana',
  TRX: 'TRON',
  BNB: 'BNB',
  POL: 'POL',
  MON: 'MON',
  ENS: 'Ethereum Name Service',
  FRAX: 'Frax',
  GRT: 'Graph Token',
  ILV: 'Illuvium',
  WOO: 'Wootrade Network',
  OCEAN: 'Ocean Token',
  CRV: 'Curve DAO Token',
};

// Display name for asset symbol (e.g. BTC -> Bitcoin).
export function balanceAssetName(assetSymbol: string): string {
  return ASSET_NAME[assetSymbol] ?? assetSymbol;
}

/**
 * Convert a human-entered decimal amount (e.g. "1.5") into the asset's smallest
 * unit as a decimal string (e.g. "1500000000000000000" for 18 decimals).
 * Throws on malformed input or more fractional digits than `decimals` allows.
 *
 * The thrown messages are internal sentinels (developer-facing only): callers
 * catch them and surface a translated, user-facing message via `useT()` —
 * never render `error.message` from here directly.
 */
export function toSmallestUnit(human: string | number, decimals: number): string {
  let humanStr = String(human).trim();
  if (humanStr === '' || humanStr === '.') throw new Error('invalid_amount');

  // Convert scientific notation to plain decimal string to avoid BigInt parsing errors
  if (humanStr.includes('e') || humanStr.includes('E')) {
    const num = Number(humanStr);
    if (isNaN(num)) throw new Error('invalid_amount');
    // Using BigInt on the number gets the exact integer if it's large without fraction
    // But human inputs are usually not scientific notation unless auto-filled.
    // A robust way for scientific notation in JS:
    const [lead, exp] = humanStr.toLowerCase().split('e');
    const expNum = parseInt(exp, 10);
    // basic expansion (this is a simplified fallback)
    if (expNum > 0) {
      const [w, f = ''] = lead.split('.');
      const missingZeros = expNum - f.length;
      if (missingZeros >= 0) {
        humanStr = w + f + '0'.repeat(missingZeros);
      } else {
        humanStr = w + f.slice(0, expNum) + '.' + f.slice(expNum);
      }
    }
  }

  if (!/^\d*\.?\d*$/.test(humanStr)) {
    throw new Error('invalid_amount');
  }

  const [whole, frac = ''] = humanStr.split('.');
  if (frac.length > decimals) {
    throw new Error('too_many_decimals');
  }

  try {
    const wholeBig = BigInt(whole || '0');
    const multiplier = 10n ** BigInt(decimals);
    let fracBig = 0n;
    if (frac.length > 0) {
      const paddedFrac = frac.padEnd(decimals, '0');
      fracBig = BigInt(paddedFrac);
    }
    const combined = wholeBig * multiplier + fracBig;
    return combined.toString();
  } catch {
    throw new Error('invalid_amount');
  }
}

/**
 * Format a smallest-unit decimal string back to a human amount, trimming
 * trailing fractional zeros. Inverse of {@link toSmallestUnit}.
 */
export function fromSmallestUnit(raw: string | number | bigint, decimals: number): string {
  let value: bigint;
  try {
    if (
      typeof raw === 'number' ||
      (typeof raw === 'string' && raw.toString().toLowerCase().includes('e'))
    ) {
      value = BigInt(Math.floor(Number(raw)));
    } else {
      value = BigInt(raw);
    }
  } catch {
    return String(raw);
  }

  if (decimals === 0) return value.toString();

  const divisor = 10n ** BigInt(decimals);
  const whole = value / divisor;
  const frac = value % divisor;

  if (frac === 0n) return whole.toString();

  const fracStr = frac.toString().padStart(decimals, '0').replace(/0+$/, '');
  return `${whole.toString()}.${fracStr}`;
}

/**
 * Formats balance to 8 decimal places e.g. 1.00000000.
 * Balances below 0.00000001 display as <0.00000001.
 */
export function formatDisplayBalance(formatted: string | number): string {
  const str = String(formatted).trim();
  if (!str || str === 'NaN' || str === '0') return '0';

  // Handle scientific notation for small numbers (e.g. from Number.toString) if any
  let normalized = str;
  if (normalized.toLowerCase().includes('e')) {
    try {
      normalized = Number(normalized).toFixed(20).replace(/0+$/, '').replace(/\.$/, '');
    } catch {
      return str;
    }
  }

  // Handle very small numbers without scientific notation
  if (normalized.startsWith('0.') && normalized !== '0.0') {
    const fracPart = normalized.split('.')[1] || '';
    if (fracPart.length >= 8 && /^0{7,}/.test(fracPart)) {
      // if it's less than 0.00000001
      const num = Number(normalized);
      if (num > 0 && num < 0.00000001) {
        return '<0.00000001';
      }
    }
  }

  const [whole = '0', frac = ''] = normalized.split('.');

  if (!frac) {
    return `${whole}.00000000`;
  }

  return `${whole}.${frac.slice(0, 8).padEnd(8, '0')}`;
}
