import {
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  unstable_defaultDirectiveFormatter,
  useAui,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { MOCK_MEMORY_RECALL } from '../../../pages/dev/assistant-ui-demo/assistantUiMock/mockScript';
import { callCoreRpc } from '../../../services/coreRpcClient';
import chatRuntimeReducer, { type ArtifactSnapshot } from '../../../store/chatRuntimeSlice';
import type { Chunk } from '../../../utils/tauriCommands/memoryTree';
import {
  fileMentionsFromArtifacts,
  memoryMentionsFromChunks,
  trailingMentionQuery,
  useMentionSource,
} from './useMentionSource';

vi.mock('../../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

function chunk(id: string, preview: string): Chunk {
  return {
    id,
    source_kind: 'email',
    source_id: 'thread-9',
    owner: 'me',
    timestamp_ms: 0,
    token_count: 10,
    lifecycle_status: 'admitted',
    content_preview: preview,
    has_embedding: true,
    tags: [],
  } as Chunk;
}

const READY: ArtifactSnapshot = {
  artifactId: 'art-1',
  kind: 'document',
  title: 'Signed contract',
  status: 'ready',
  path: 'artifacts/signed-contract.docx',
  updatedAt: 1,
};
const PENDING: ArtifactSnapshot = {
  artifactId: 'art-2',
  kind: 'document',
  title: 'Draft',
  status: 'in_progress',
  updatedAt: 1,
};

function setup() {
  const base = chatRuntimeReducer(undefined, { type: '@@test/init' });
  const store = configureStore({
    reducer: combineReducers({ chatRuntime: chatRuntimeReducer }),
    preloadedState: { chatRuntime: { ...base, artifactsByThread: { t1: [READY, PENDING] } } },
  });
  const messages: ThreadMessageLike[] = [];
  function Runtime({ children }: { children: ReactNode }) {
    const runtime = useExternalStoreRuntime({
      messages,
      convertMessage: (m: ThreadMessageLike) => m,
      onNew: async () => {},
    });
    return <AssistantRuntimeProvider runtime={runtime}>{children}</AssistantRuntimeProvider>;
  }
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store}>
      <Runtime>{children}</Runtime>
    </Provider>
  );
  return renderHook(() => ({ source: useMentionSource('t1'), aui: useAui() }), { wrapper });
}

describe('trailingMentionQuery', () => {
  it('reads the query of a trailing @mention', () => {
    expect(trailingMentionQuery('ask @design')).toBe('design');
    expect(trailingMentionQuery('@')).toBe('');
  });

  it('ignores text that does not end in a mention, and emails', () => {
    expect(trailingMentionQuery('ask @design now')).toBeNull();
    expect(trailingMentionQuery('mail me@example')).toBeNull();
  });
});

describe('memoryMentionsFromChunks', () => {
  it('builds a single-line label that survives the directive syntax', () => {
    const [mention] = memoryMentionsFromChunks([
      chunk('c1', 'Design [sync]\nnotes {v2} with a very long tail that keeps going on'),
    ]);
    expect(mention).toMatchObject({
      id: 'c1',
      type: 'memory',
      description: 'email',
      icon: 'memory',
    });
    expect(mention!.label).toBe('Design sync notes v2 with a very long tail that…');

    const text = unstable_defaultDirectiveFormatter.serialize(mention!);
    expect(unstable_defaultDirectiveFormatter.parse(text)).toEqual([
      { kind: 'mention', type: 'memory', label: mention!.label, id: 'c1' },
    ]);
  });

  it('maps the dev recall fixture to one memory mention per chunk', () => {
    const mentions = memoryMentionsFromChunks(MOCK_MEMORY_RECALL.chunks);
    expect(mentions.map(m => m.id)).toEqual(MOCK_MEMORY_RECALL.chunks.map(c => c.id));
    expect(mentions.every(m => m.label.length > 0)).toBe(true);
  });

  it('falls back to the source id when a chunk has no preview', () => {
    const [mention] = memoryMentionsFromChunks([
      { ...chunk('c2', ''), content_preview: undefined },
    ]);
    expect(mention!.label).toBe('thread-9');
  });
});

describe('fileMentionsFromArtifacts', () => {
  it('lists only ready artifacts', () => {
    expect(fileMentionsFromArtifacts([READY, PENDING])).toEqual([
      {
        id: 'art-1',
        type: 'file',
        label: 'Signed contract',
        description: 'artifacts/signed-contract.docx',
        icon: 'files',
      },
    ]);
  });
});

describe('useMentionSource', () => {
  beforeEach(() => vi.mocked(callCoreRpc).mockReset());

  it('offers Memory and Files categories, with the thread files listed', () => {
    const { result } = setup();
    const { adapter, directive } = result.current.source;
    expect(adapter.categories()).toEqual([
      { id: 'memory', label: 'Memory' },
      { id: 'files', label: 'Files' },
    ]);
    expect(adapter.categoryItems('files').map(i => i.id)).toEqual(['art-1']);
    expect(directive.formatter).toBe(unstable_defaultDirectiveFormatter);
    expect(callCoreRpc).not.toHaveBeenCalled();
  });

  it('searches memory recall for a typed @query and surfaces the hits', async () => {
    vi.mocked(callCoreRpc).mockResolvedValue({
      result: { chunks: [chunk('c1', 'Quarterly planning notes')], scores: [0.9] },
    });
    const { result } = setup();

    act(() => result.current.aui.composer.setText('what about @quart'));
    await waitFor(() => expect(result.current.source.isLoading).toBe(false));
    await waitFor(() =>
      expect(result.current.source.adapter.search?.('quart').map(i => i.id)).toContain('c1')
    );
    expect(callCoreRpc).toHaveBeenCalledWith({
      method: 'openhuman.memory_tree_recall',
      params: { query: 'quart', k: 8 },
    });
    expect(result.current.source.adapter.categoryItems('memory').map(i => i.id)).toEqual(['c1']);
  });

  it('keeps semantic hits even when their label does not contain the query', async () => {
    vi.mocked(callCoreRpc).mockResolvedValue({
      chunks: [chunk('c3', 'Roadmap review')],
      scores: [0.7],
    });
    const { result } = setup();
    act(() => result.current.aui.composer.setText('@planning'));
    await waitFor(() =>
      expect(result.current.source.adapter.search?.('planning').map(i => i.id)).toEqual(['c3'])
    );
  });

  it('treats a failed recall as no memory hits', async () => {
    vi.mocked(callCoreRpc).mockImplementation(async request => {
      if (request?.method === 'openhuman.memory_tree_recall') {
        throw new Error('memory tree disabled');
      }
      return {} as never;
    });
    const { result } = setup();
    act(() => result.current.aui.composer.setText('@roadmap'));
    await waitFor(() => expect(callCoreRpc).toHaveBeenCalled());
    await waitFor(() => expect(result.current.source.isLoading).toBe(false));
    expect(result.current.source.adapter.categoryItems('memory')).toEqual([]);
  });
});
