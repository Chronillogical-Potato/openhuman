import { beforeEach, describe, expect, it, vi } from 'vitest';

import { callCoreRpc } from '../coreRpcClient';
import {
  DOCUMENT_SOURCE_KINDS,
  isMemoryOn,
  memoryAgentsList,
  memoryBrainIngest,
  memoryEngineGet,
  memoryEngineSet,
  memoryEnginesList,
  memoryErrorCode,
  memoryErrorMessage,
  memoryFetch,
  memoryImportRetryFailed,
  memoryImportScan,
  memoryImportStart,
  memoryImportStatus,
  memoryItemsList,
  memoryPolicyGet,
  memoryPolicySet,
  memorySourcesAdd,
  memorySourcesList,
  memorySourcesRemove,
  memorySourcesSync,
} from './memoryApi';

vi.mock('../coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

const rpc = vi.mocked(callCoreRpc);

beforeEach(() => {
  rpc.mockReset();
  rpc.mockResolvedValue({});
});

describe('memoryApi wire calls', () => {
  // [call, expected method, expected params]
  const cases: Array<[string, () => Promise<unknown>, string, Record<string, unknown>]> = [
    ['engines list', () => memoryEnginesList(), 'openhuman.memory_engines_list', {}],
    ['engine get', () => memoryEngineGet(), 'openhuman.memory_engine_get', {}],
    [
      'engine set',
      () => memoryEngineSet({ engine: 'cortexdb', endpoint: 'https://x', api_key: 'k' }),
      'openhuman.memory_engine_set',
      { engine: 'cortexdb', endpoint: 'https://x', api_key: 'k' },
    ],
    [
      'fetch drops undefined params',
      () => memoryFetch({ query: 'q', mode: undefined, limit: 5 }),
      'openhuman.memory_fetch',
      { query: 'q', limit: 5 },
    ],
    [
      'items list',
      () => memoryItemsList({ filter: { kinds: ['learning'] }, limit: 20, cursor: 'c' }),
      'openhuman.memory_items_list',
      { filter: { kinds: ['learning'] }, limit: 20, cursor: 'c' },
    ],
    ['sources list', () => memorySourcesList(), 'openhuman.memory_sources_list', {}],
    [
      'sources add',
      () => memorySourcesAdd({ kind: 'folder', target: '/notes', schedule_mins: 60 }),
      'openhuman.memory_sources_add',
      { kind: 'folder', target: '/notes', schedule_mins: 60 },
    ],
    [
      'sources remove',
      () => memorySourcesRemove('s1', true),
      'openhuman.memory_sources_remove',
      { id: 's1', forget_items: true },
    ],
    ['sources sync all', () => memorySourcesSync(), 'openhuman.memory_sources_sync', {}],
    [
      'sources sync one',
      () => memorySourcesSync('s1'),
      'openhuman.memory_sources_sync',
      { id: 's1' },
    ],
    ['policy get', () => memoryPolicyGet(), 'openhuman.memory_policy_get', {}],
    [
      'policy set',
      () => memoryPolicySet({ log_conversations: false, budget_tokens: 2000 }),
      'openhuman.memory_policy_set',
      { log_conversations: false, budget_tokens: 2000 },
    ],
    ['agents list', () => memoryAgentsList(), 'openhuman.memory_agents_list', {}],
    [
      'brain ingest',
      () => memoryBrainIngest({ text: 'hello', title: 'Note' }),
      'openhuman.memory_brain_ingest',
      { text: 'hello', title: 'Note' },
    ],
    ['import scan', () => memoryImportScan(), 'openhuman.memory_import_scan', {}],
    [
      'import start always sends consent',
      () => memoryImportStart(),
      'openhuman.memory_import_start',
      { consent: true },
    ],
    ['import status', () => memoryImportStatus(), 'openhuman.memory_import_status', {}],
    [
      'import retry failed',
      () => memoryImportRetryFailed(),
      'openhuman.memory_import_retry_failed',
      {},
    ],
  ];

  it.each(cases)('%s', async (_name, invoke, method, params) => {
    await invoke();
    expect(rpc).toHaveBeenCalledWith({ method, params });
  });
});

describe('document source kinds', () => {
  it('offers only folder and file', () => {
    expect(DOCUMENT_SOURCE_KINDS).toEqual(['folder', 'file']);
  });
});

describe('memoryApi responses', () => {
  it('returns the payload as-is', async () => {
    const state = { engine: 'tinyhumans', has_key: false, status: 'ok', fetch_modes: ['hybrid'] };
    rpc.mockResolvedValue(state);
    await expect(memoryEngineGet()).resolves.toEqual(state);
  });

  it('unwraps the { result, logs } controller envelope', async () => {
    rpc.mockResolvedValue({ result: { sources: [] }, logs: ['x'] });
    await expect(memorySourcesList()).resolves.toEqual({ sources: [] });
  });

  it('rethrows RPC failures', async () => {
    rpc.mockRejectedValue(new Error('MEMORY_OFF: no engine'));
    await expect(memoryFetch({ query: 'q' })).rejects.toThrow('MEMORY_OFF');
  });
});

describe('memoryErrorCode', () => {
  it('reads data.code', () => {
    expect(memoryErrorCode({ message: 'x', data: { code: 'UNSUPPORTED' } })).toBe('UNSUPPORTED');
  });

  it('reads data.kind', () => {
    expect(memoryErrorCode({ message: 'x', data: { kind: 'MEMORY_OFF' } })).toBe('MEMORY_OFF');
  });

  it('falls back to a code prefix on the message', () => {
    expect(memoryErrorCode(new Error('UNAUTHORIZED: bad key'))).toBe('UNAUTHORIZED');
    expect(memoryErrorCode({ message: 'x', data: { code: 'INSUFFICIENT_CREDITS' } })).toBe(
      'INSUFFICIENT_CREDITS'
    );
    expect(memoryErrorCode(new Error('UNAVAILABLE: timed out'))).toBe('UNAVAILABLE');
  });

  it('returns null for an unrelated error', () => {
    expect(memoryErrorCode(new Error('boom'))).toBeNull();
    expect(memoryErrorCode({ data: { code: 'OTHER' } })).toBeNull();
    expect(memoryErrorCode(null)).toBeNull();
  });
});

describe('helpers', () => {
  it('memoryErrorMessage handles errors, objects and primitives', () => {
    expect(memoryErrorMessage(new Error('a'))).toBe('a');
    expect(memoryErrorMessage({ message: 'b' })).toBe('b');
    expect(memoryErrorMessage('c')).toBe('c');
  });

  it('memoryErrorMessage explains an account-wide refusal when given a translator', () => {
    const t = (key: string) => `t:${key}`;
    const credits = {
      message: 'insufficient credits: [USER_INSUFFICIENT_CREDITS] memory API recall (HTTP 402)',
      data: { code: 'INSUFFICIENT_CREDITS' },
    };
    expect(memoryErrorMessage(credits, t)).toBe('t:memory.error.insufficientCredits');
    expect(memoryErrorMessage({ message: 'x', data: { code: 'UNAVAILABLE' } }, t)).toBe(
      't:memory.error.unavailable'
    );
    // A rejected key and an engine fault keep their own message, and without
    // `t` nothing changes.
    expect(memoryErrorMessage(new Error('UNAUTHORIZED: bad key'), t)).toBe('UNAUTHORIZED: bad key');
    expect(memoryErrorMessage({ message: 'boom', data: { code: 'ENGINE' } }, t)).toBe('boom');
    expect(memoryErrorMessage(credits)).toBe(credits.message);
  });

  it('isMemoryOn needs an engine that is not off', () => {
    expect(isMemoryOn(null)).toBe(false);
    expect(isMemoryOn({ engine: null, has_key: false, status: 'off', fetch_modes: [] })).toBe(
      false
    );
    expect(isMemoryOn({ engine: 'cortexdb', has_key: true, status: 'off', fetch_modes: [] })).toBe(
      false
    );
    expect(
      isMemoryOn({ engine: 'cortexdb', has_key: true, status: 'degraded', fetch_modes: [] })
    ).toBe(true);
  });
});
