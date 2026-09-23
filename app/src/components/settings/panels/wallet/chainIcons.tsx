import baseIcon from '../../../../assets/icons/base.svg';
import bitcoinIcon from '../../../../assets/icons/bitcoin.svg';
import bscIcon from '../../../../assets/icons/bsc.svg';
import ethereumIcon from '../../../../assets/icons/ethereum.svg';
import networkBaseIcon from '../../../../assets/icons/network_base.svg';
import networkBitcoinIcon from '../../../../assets/icons/network_bitcoin.svg';
import networkBscIcon from '../../../../assets/icons/network_bsc.svg';
import networkEthIcon from '../../../../assets/icons/network_eth.svg';
import networkSolanaIcon from '../../../../assets/icons/network_solana.svg';
import networkTronIcon from '../../../../assets/icons/network_tron.svg';
import solanaIcon from '../../../../assets/icons/solana.svg';
import tokenBnbIcon from '../../../../assets/icons/tokens/bnb.svg';
import tokenBtcIcon from '../../../../assets/icons/tokens/btc.svg';
import tokenEthIcon from '../../../../assets/icons/tokens/eth.svg';
import tokenSolIcon from '../../../../assets/icons/tokens/sol.svg';
import tokenTronIcon from '../../../../assets/icons/tokens/tron.svg';
import tronIcon from '../../../../assets/icons/tron.svg';
import { type EvmNetwork, type WalletChain } from '../../../../services/walletApi';

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

export const TOKEN_ICONS: Record<string, string> = {
  ETH: tokenEthIcon,
  BNB: tokenBnbIcon,
  BTC: tokenBtcIcon,
  SOL: tokenSolIcon,
  TRX: tokenTronIcon,
};

export function ChainIcon({ chain, evmNetwork }: { chain: WalletChain; evmNetwork?: EvmNetwork }) {
  const src = CHAIN_ICONS[evmNetwork || chain];
  if (!src) return null;
  return <img src={src} alt="" aria-hidden className="w-10 h-10 shrink-0 object-contain" />;
}
