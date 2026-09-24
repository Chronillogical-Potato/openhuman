import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { getCoreStateSnapshot, setCoreStateSnapshot } from '../../../lib/coreState/store';
import { setStatusForUser } from '../../../store/socketSlice';
import { createTestStore, renderWithProviders } from '../../../test/test-utils';
import { ConnectionStateBanner, RESUMED_VISIBLE_MS } from './ConnectionStateBanner';

const connect = vi.hoisted(() => vi.fn());
vi.mock('../../../services/socketService', () => ({ socketService: { connect } }));

// `selectSocketStatus` keys the socket slice by the core snapshot's user id,
// which is unset in tests, so the socket writes under the pending id.
const USER = '__pending__';
type Status = 'connected' | 'disconnected' | 'connecting';

function setup(initial: Status) {
  const store = createTestStore();
  store.dispatch(setStatusForUser({ userId: USER, status: initial }));
  const view = renderWithProviders(<ConnectionStateBanner />, { store });
  const setStatus = (status: Status) =>
    act(() => {
      store.dispatch(setStatusForUser({ userId: USER, status }));
    });
  return { ...view, setStatus };
}

const banner = () => screen.queryByTestId('connection-state-banner');

describe('ConnectionStateBanner', () => {
  const originalSnapshot = getCoreStateSnapshot();

  beforeEach(() => {
    connect.mockClear();
  });

  afterEach(() => {
    setCoreStateSnapshot(originalSnapshot);
    vi.useRealTimers();
  });

  it('renders nothing while the socket is connected', () => {
    setup('connected');
    expect(banner()).toBeNull();
  });

  it('stays quiet before the socket has ever connected (app boot)', () => {
    const { setStatus } = setup('disconnected');
    expect(banner()).toBeNull();
    setStatus('connecting');
    expect(banner()).toBeNull();
  });

  it('shows the dropped state with a Reconnect action once a live socket drops', () => {
    const { setStatus } = setup('connected');
    setStatus('disconnected');
    expect(banner()).toHaveTextContent('Connection lost. The run kept going on the server.');
    expect(screen.getByRole('button', { name: 'Reconnect' })).toBeInTheDocument();
  });

  it('shows the reconnecting state while a dropped socket is reconnecting', () => {
    const { setStatus } = setup('connected');
    setStatus('disconnected');
    setStatus('connecting');
    expect(banner()).toHaveTextContent('Reconnecting');
    expect(screen.queryByRole('button', { name: 'Reconnect' })).toBeNull();
  });

  it('shows the resumed state after reconnecting, then clears it', () => {
    vi.useFakeTimers();
    const { setStatus } = setup('connected');
    setStatus('disconnected');
    setStatus('connecting');
    setStatus('connected');
    expect(banner()).toHaveTextContent('Picked the stream back up.');

    act(() => {
      vi.advanceTimersByTime(RESUMED_VISIBLE_MS);
    });
    expect(banner()).toBeNull();
  });

  it('drops straight back from resumed if the socket drops again', () => {
    vi.useFakeTimers();
    const { setStatus } = setup('connected');
    setStatus('disconnected');
    setStatus('connected');
    setStatus('disconnected');
    act(() => {
      vi.advanceTimersByTime(RESUMED_VISIBLE_MS);
    });
    expect(banner()).toHaveTextContent('Connection lost.');
  });

  it('Reconnect reconnects the socket with the current session token', () => {
    setCoreStateSnapshot({
      ...originalSnapshot,
      snapshot: { ...originalSnapshot.snapshot, sessionToken: 'session-jwt' },
    });
    const { setStatus } = setup('connected');
    setStatus('disconnected');
    fireEvent.click(screen.getByRole('button', { name: 'Reconnect' }));
    expect(connect).toHaveBeenCalledTimes(1);
    expect(connect).toHaveBeenCalledWith('session-jwt');
  });

  it('Reconnect does nothing without a session token', () => {
    setCoreStateSnapshot({
      ...originalSnapshot,
      snapshot: { ...originalSnapshot.snapshot, sessionToken: null },
    });
    const { setStatus } = setup('connected');
    setStatus('disconnected');
    fireEvent.click(screen.getByRole('button', { name: 'Reconnect' }));
    expect(connect).not.toHaveBeenCalled();
  });

  it('renders nothing under a host store that has no socket slice', () => {
    const store = configureStore({ reducer: { other: (state: number = 0) => state } });
    const { container } = render(
      <Provider store={store}>
        <ConnectionStateBanner />
      </Provider>
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing outside a Redux store (standalone thread renders)', () => {
    const { container } = render(<ConnectionStateBanner />);
    expect(container).toBeEmptyDOMElement();
  });
});
