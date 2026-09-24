import {
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { act, render, screen } from '@testing-library/react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';

import { setStatusForUser } from '../../store/socketSlice';
import { createTestStore } from '../../test/test-utils';
import { Thread } from './thread';

vi.mock('../../services/socketService', () => ({ socketService: { connect: vi.fn() } }));

/** The connection banner sits in the viewport footer, directly above the composer. */
function Harness() {
  const messages: ThreadMessageLike[] = [];
  const runtime = useExternalStoreRuntime({
    messages,
    convertMessage: (m: ThreadMessageLike) => m,
    onNew: async () => {},
  });
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <Thread />
    </AssistantRuntimeProvider>
  );
}

describe('thread connection-state banner', () => {
  it('renders above the composer once the socket drops', () => {
    const store = createTestStore();
    store.dispatch(setStatusForUser({ userId: '__pending__', status: 'connected' }));
    const { container } = render(
      <Provider store={store}>
        <Harness />
      </Provider>
    );
    expect(screen.queryByTestId('connection-state-banner')).toBeNull();

    act(() => {
      store.dispatch(setStatusForUser({ userId: '__pending__', status: 'disconnected' }));
    });

    const banner = screen.getByTestId('connection-state-banner');
    const composer = container.querySelector('.aui-composer-root, [data-slot="aui_composer-root"]');
    expect(composer).not.toBeNull();
    expect(banner.compareDocumentPosition(composer as Node)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
  });
});
