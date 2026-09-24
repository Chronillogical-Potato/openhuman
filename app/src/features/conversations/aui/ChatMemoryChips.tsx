/**
 * Memory tool calls (`memory_store`, `memory_recall`, `memory_hybrid_search`)
 * rendered through the vendored `memory-chips` element instead of the raw
 * JSON `ToolDataView` fallback every other dynamic tool gets.
 *
 * There is no wire event yet for "memory stored/recalled during this turn"
 * (`memory_activity`, planned per `scratchpad/wire-contract.md`'s "Planned
 * new socket events" list — not emitted by any core workstream as of this
 * pass), so this renders from the tool call's own `args`/`result` — which the
 * core already sends for every tool call — rather than a side-channel event.
 * Once `memory_activity` lands, a second slot (turn-level chips under the
 * settled answer, independent of any one tool call) can read it the same way
 * `ChatSources`/`SourceGroup` reads `citations`.
 *
 * The core's `memory_store` / `memory_recall` / `memory_hybrid_search`
 * argument and result shapes are not pinned by a wire contract at this pass,
 * so field access here is deliberately lenient (`asRecord`, optional
 * chaining, safe fallbacks) and documented per tool rather than typed against
 * a contract that does not exist yet.
 */
import type { ToolCallMessagePartComponent } from '@assistant-ui/react';

import {
  type MemoryChip,
  MemoryChips,
} from '../../../components/assistant-ui/elements/memory-chips';
import { useT } from '../../../lib/i18n/I18nContext';

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === 'object' ? (value as Record<string, unknown>) : undefined;
}

function stringField(record: Record<string, unknown> | undefined, key: string): string | undefined {
  const value = record?.[key];
  return typeof value === 'string' && value.length > 0 ? value : undefined;
}

/** `memory_store`: one write, keyed by its `key`/`category` argument. */
function chipsForStore(args: unknown): MemoryChip[] {
  const record = asRecord(args);
  const key = stringField(record, 'key') ?? stringField(record, 'category');
  const text = key ?? stringField(record, 'content')?.slice(0, 60);
  if (!text) return [];
  return [{ id: `store:${text}`, text, change: 'added' }];
}

/** `memory_recall` / `memory_hybrid_search`: each hit in the result list. */
function chipsForRecall(result: unknown): MemoryChip[] {
  const record = asRecord(result);
  const items = Array.isArray(result)
    ? result
    : Array.isArray(record?.items)
      ? (record?.items as unknown[])
      : Array.isArray(record?.results)
        ? (record?.results as unknown[])
        : [];
  return items.flatMap((item, index): MemoryChip[] => {
    const entry = asRecord(item);
    const text =
      stringField(entry, 'key') ?? stringField(entry, 'text') ?? stringField(entry, 'snippet');
    if (!text) return [];
    return [{ id: `recall:${index}:${text}`, text: text.slice(0, 60), change: 'existing' }];
  });
}

const CHIP_BUILDERS: Record<string, (args: unknown, result: unknown) => MemoryChip[]> = {
  memory_store: args => chipsForStore(args),
  memory_recall: (_args, result) => chipsForRecall(result),
  memory_hybrid_search: (_args, result) => chipsForRecall(result),
};

export function memoryToolChips(toolName: string, args: unknown, result: unknown): MemoryChip[] {
  return CHIP_BUILDERS[toolName]?.(args, result) ?? [];
}

function createMemoryToolCall(toolName: string): ToolCallMessagePartComponent {
  const MemoryToolCall: ToolCallMessagePartComponent = ({ args, result }) => {
    const { t } = useT();
    const chips = memoryToolChips(toolName, args, result);
    if (chips.length === 0) return null;
    return (
      <MemoryChips
        chips={chips}
        headingRememberedLabel={n =>
          t('conversations.memoryChips.remembered').replace('{n}', String(n))
        }
        headingIdleLabel={t('conversations.memoryChips.idle')}
        forgetAriaLabel={text =>
          t('conversations.memoryChips.forgetAriaLabel').replace('{text}', text)
        }
      />
    );
  };
  return MemoryToolCall;
}

/** One toolkit entry per memory tool name, sharing the same renderer logic. */
export const MemoryStoreCall = createMemoryToolCall('memory_store');
export const MemoryRecallCall = createMemoryToolCall('memory_recall');
export const MemoryHybridSearchCall = createMemoryToolCall('memory_hybrid_search');
