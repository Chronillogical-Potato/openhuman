/**
 * Frontend client for `openhuman.agent_context_breakdown` — where an agent
 * turn's fixed prompt budget goes: the rendered system-prompt sections, the
 * advertised tool schemas (one `tools` row) and, with a thread id, that
 * thread's persisted history (one `history` row).
 *
 * The core call is expensive on a cold cache (it rebuilds the agent), so the
 * composer only asks for it when the user opens the breakdown; see
 * `features/conversations/aui/ContextUsage.tsx`.
 */
import debug from 'debug';

import { callCoreRpc } from '../coreRpcClient';

const log = debug('openhuman:agentContextApi');

const METHOD = 'openhuman.agent_context_breakdown';

/** One labelled slice of the prompt budget, as the core measured it. */
export interface ContextBreakdownSection {
  label: string;
  bytes: number;
  est_tokens: number;
}

export interface ContextBreakdown {
  sections: ContextBreakdownSection[];
  total_est_tokens: number;
  /** The resolved model's window in tokens; `0` when the core does not know it. */
  context_window: number;
}

const count = (value: unknown): number =>
  typeof value === 'number' && Number.isFinite(value) ? Math.max(0, value) : 0;

function isSection(value: unknown): value is ContextBreakdownSection {
  if (!value || typeof value !== 'object') return false;
  const section = value as Record<string, unknown>;
  return typeof section.label === 'string' && typeof section.est_tokens === 'number';
}

/** Accept the bare response or the `{ result, logs }` RpcOutcome envelope. */
function unwrap(response: unknown): Record<string, unknown> | null {
  if (!response || typeof response !== 'object') return null;
  const record = response as Record<string, unknown>;
  if ('result' in record && record.result && typeof record.result === 'object') {
    return record.result as Record<string, unknown>;
  }
  return record;
}

/**
 * Measure the prompt budget of the orchestrator turn for `threadId` (or, with
 * no thread yet, the fixed prefix alone). Rejects when the core answers
 * without a `sections` list — an older core that lacks the method.
 */
export async function getContextBreakdown(threadId: string | null): Promise<ContextBreakdown> {
  log('context_breakdown thread=%s', threadId ?? '(none)');
  const response = await callCoreRpc<unknown>({
    method: METHOD,
    params: threadId ? { thread_id: threadId } : {},
  });
  const value = unwrap(response);
  if (!value || !Array.isArray(value.sections)) {
    log('context_breakdown: no sections in response');
    throw new Error(`${METHOD} returned no sections`);
  }
  return {
    sections: value.sections
      .filter(isSection)
      .map(section => ({
        label: section.label,
        bytes: count(section.bytes),
        est_tokens: count(section.est_tokens),
      })),
    total_est_tokens: count(value.total_est_tokens),
    context_window: count(value.context_window),
  };
}
