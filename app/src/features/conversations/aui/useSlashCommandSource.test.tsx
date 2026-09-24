import {
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  useAui,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { registry } from '../../../lib/commands/registry';
import { MOCK_COMMANDS_LIST } from '../../../pages/dev/assistant-ui-demo/assistantUiMock/mockScript';
import { callCoreRpc } from '../../../services/coreRpcClient';
import runModeReducer from '../../../store/runModeSlice';
import {
  type CoreCommand,
  fetchCoreCommands,
  mergeSlashCommands,
  useSlashCommandSource,
} from './useSlashCommandSource';

vi.mock('../../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

const SKILL: CoreCommand = {
  id: 'summarize',
  label: 'Summarize',
  description: 'Summarize the thread',
  kind: 'skill',
  insert: '/summarize ',
};

function rpcByMethod(responses: Record<string, unknown>) {
  vi.mocked(callCoreRpc).mockImplementation(async request => {
    const method = request?.method ?? '';
    if (method in responses) {
      const value = responses[method];
      if (value instanceof Error) throw value;
      return value as never;
    }
    return {} as never;
  });
}

function setup({ running = false }: { running?: boolean } = {}) {
  const store = configureStore({ reducer: combineReducers({ runMode: runModeReducer }) });
  const onCancel = vi.fn(async () => {});
  const messages: ThreadMessageLike[] = [];
  function Runtime({ children }: { children: ReactNode }) {
    const runtime = useExternalStoreRuntime({
      messages,
      isRunning: running,
      convertMessage: (m: ThreadMessageLike) => m,
      onNew: async () => {},
      onCancel,
    });
    return <AssistantRuntimeProvider runtime={runtime}>{children}</AssistantRuntimeProvider>;
  }
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store}>
      <Runtime>{children}</Runtime>
    </Provider>
  );
  const hook = renderHook(() => ({ source: useSlashCommandSource('t1'), aui: useAui() }), {
    wrapper,
  });
  return { store, onCancel, ...hook };
}

function execute(result: ReturnType<typeof setup>['result'], id: string) {
  const item = result.current.source.adapter.search?.('').find(i => i.id === id);
  expect(item, `command ${id} is offered`).toBeDefined();
  act(() => result.current.source.action.onExecute(item!));
}

describe('fetchCoreCommands', () => {
  beforeEach(() => vi.mocked(callCoreRpc).mockReset());

  it('unwraps the registry envelope and drops malformed entries', async () => {
    rpcByMethod({
      'openhuman.commands_list': {
        data: { commands: [SKILL, { id: 'x' }, { id: 'w', label: 'W', kind: 'workflow' }] },
      },
    });
    await expect(fetchCoreCommands()).resolves.toEqual([
      SKILL,
      { id: 'w', label: 'W', kind: 'workflow' },
    ]);
  });

  it('accepts the dev fixture as a well-formed commands_list response', async () => {
    rpcByMethod({ 'openhuman.commands_list': { data: { commands: MOCK_COMMANDS_LIST } } });
    await expect(fetchCoreCommands()).resolves.toEqual(MOCK_COMMANDS_LIST);
  });

  it('accepts a bare array', async () => {
    rpcByMethod({ 'openhuman.commands_list': [SKILL] });
    await expect(fetchCoreCommands()).resolves.toEqual([SKILL]);
  });

  it('falls back to an empty list when the core lacks the method', async () => {
    rpcByMethod({ 'openhuman.commands_list': new Error('unknown method: commands_list') });
    await expect(fetchCoreCommands()).resolves.toEqual([]);
  });
});

describe('mergeSlashCommands', () => {
  const builtin = { id: 'plan', description: 'Local plan', execute: vi.fn() };

  it('keeps local builtins over core duplicates, then core, then registry commands', () => {
    const insert = vi.fn();
    const merged = mergeSlashCommands({
      builtins: [builtin],
      core: [{ id: 'plan', label: 'Plan', kind: 'builtin', description: 'Core plan' }, SKILL],
      registry: [
        { id: 'summarize', execute: vi.fn() },
        { id: 'palette', description: 'Open palette', execute: vi.fn() },
      ],
      insert,
    });

    expect(merged.map(c => c.id)).toEqual(['plan', 'summarize', 'palette']);
    expect(merged[0]?.description).toBe('Local plan');
    expect(merged[1]).toMatchObject({ description: 'Summarize the thread', icon: 'skill' });

    merged[1]?.execute();
    expect(insert).toHaveBeenCalledWith('/summarize ');
  });

  it('inserts `/id ` for a core command without an explicit insert text', () => {
    const insert = vi.fn();
    const [command] = mergeSlashCommands({
      builtins: [],
      core: [{ id: 'deploy', label: 'Deploy', kind: 'workflow' }],
      registry: [],
      insert,
    });
    expect(command?.description).toBe('Deploy');
    command?.execute();
    expect(insert).toHaveBeenCalledWith('/deploy ');
  });
});

describe('useSlashCommandSource', () => {
  beforeEach(() => {
    vi.mocked(callCoreRpc).mockReset();
    registry.reset();
  });
  afterEach(() => registry.reset());

  it('offers the builtins with translated descriptions and English popover labels', async () => {
    rpcByMethod({ 'openhuman.commands_list': new Error('missing') });
    const { result } = setup();
    await waitFor(() => expect(result.current.source.isLoading).toBe(false));

    const items = result.current.source.adapter.search?.('') ?? [];
    expect(items.map(i => i.id)).toEqual(['new', 'clear', 'stop', 'plan', 'build']);
    expect(items.find(i => i.id === 'plan')?.description).toBe(
      'Plan first: review the steps before anything runs'
    );
    expect(result.current.source.emptyItemsLabel).toBe('No matching commands');
    expect(result.current.source.action.removeOnExecute).toBe(true);
  });

  it('adds core skills from commands.list once it resolves', async () => {
    rpcByMethod({ 'openhuman.commands_list': { data: { commands: [SKILL] } } });
    const { result } = setup();
    await waitFor(() =>
      expect(result.current.source.adapter.search?.('summ').map(i => i.id)).toEqual(['summarize'])
    );
  });

  it('switches the run mode for /plan and /build', async () => {
    rpcByMethod({});
    const { result, store } = setup();

    execute(result, 'plan');
    await waitFor(() =>
      expect(callCoreRpc).toHaveBeenCalledWith({
        method: 'openhuman.agent_set_run_mode',
        params: { thread_id: 't1', mode: 'plan' },
      })
    );
    expect(store.getState().runMode.byThread.t1).toBe('plan');

    execute(result, 'build');
    await waitFor(() => expect(store.getState().runMode.byThread.t1).toBe('build'));
  });

  it('cancels the running turn for /stop', async () => {
    rpcByMethod({});
    const { result, onCancel } = setup({ running: true });
    execute(result, 'stop');
    await waitFor(() => expect(onCancel).toHaveBeenCalledOnce());
  });

  it('runs the existing new-chat action for /new and /clear', async () => {
    rpcByMethod({});
    const frame = Symbol('global');
    const newChat = vi.fn();
    registry.setActiveStack([frame]);
    registry.registerAction({ id: 'chat.new', label: 'New chat', handler: newChat }, frame);
    const { result } = setup();

    execute(result, 'new');
    execute(result, 'clear');
    expect(newChat).toHaveBeenCalledTimes(2);
  });

  it('inserts a core skill command into the composer', async () => {
    rpcByMethod({ 'openhuman.commands_list': [SKILL] });
    const { result } = setup();
    await waitFor(() =>
      expect(result.current.source.adapter.search?.('summ').length).toBeGreaterThan(0)
    );

    execute(result, 'summarize');
    expect(result.current.aui.composer.getState().text).toBe('/summarize ');
  });
});
