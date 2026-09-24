import { useEffect, useState } from 'react';

/**
 * Live "expires in Ns" text for a parked approval's TTL, ticking once a
 * second. Shared by every approval surface (in-thread gated tool call,
 * out-of-thread flow/unrouted/flow-run banners) so the countdown format never
 * drifts between them.
 *
 * Returns `null` once expired or when `expiresAt` is absent/unparseable —
 * callers render nothing in that case rather than a negative countdown; the
 * `approval_decided` socket event (`resolvePendingApprovalForThread`) is what
 * actually resolves an expired gate, not this timer.
 */
export function useApprovalExpirySeconds(expiresAt: string | null | undefined): number | null {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!expiresAt) return;
    const id = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(id);
  }, [expiresAt]);

  if (!expiresAt) return null;
  const expiresAtMs = Date.parse(expiresAt);
  if (Number.isNaN(expiresAtMs)) return null;
  const remaining = Math.round((expiresAtMs - now) / 1_000);
  return remaining > 0 ? remaining : null;
}

/** `125` -> `2:05`; `45` -> `0:45`. Locale-neutral (digits + `:` only). */
export function formatCountdown(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return `${minutes}:${String(rest).padStart(2, '0')}`;
}
