import { describe, expect, it } from 'vitest';

import { isHostedDegraded, selectBlockingState } from '../connectivitySelectors';
import type { ConnectivityState } from '../connectivitySlice';
import type { RootState } from '../index';

const make = (over: Partial<ConnectivityState>): RootState =>
  ({
    // The selector only reads `connectivity`. Cast through unknown so we don't
    // have to fabricate the rest of the root state.
    connectivity: {
      internet: 'online',
      core: 'reachable',
      backend: 'connected',
      hosted: 'connected',
      lastError: {},
      ...over,
    },
  }) as unknown as RootState;

describe('selectBlockingState', () => {
  it('returns ok when all three channels are healthy', () => {
    expect(selectBlockingState(make({}))).toBe('ok');
  });

  it('prioritises internet outage over everything else', () => {
    expect(
      selectBlockingState(
        make({ internet: 'offline', core: 'unreachable', backend: 'disconnected' })
      )
    ).toBe('internet-offline');
  });

  it('returns core-unreachable when only the sidecar is down', () => {
    expect(selectBlockingState(make({ core: 'unreachable' }))).toBe('core-unreachable');
  });

  it('returns backend-only when just the websocket is degraded', () => {
    expect(selectBlockingState(make({ backend: 'disconnected' }))).toBe('backend-only');
    expect(selectBlockingState(make({ backend: 'connecting' }))).toBe('backend-only');
  });

  it("returns hosted-degraded when only the core's hosted link is down and retrying (#6256)", () => {
    expect(selectBlockingState(make({ hosted: 'connecting' }))).toBe('hosted-degraded');
    expect(selectBlockingState(make({ hosted: 'reconnecting' }))).toBe('hosted-degraded');
    expect(selectBlockingState(make({ hosted: 'error' }))).toBe('hosted-degraded');
  });

  it('treats a hosted link that is not running as healthy, not degraded', () => {
    // Signed out, local session, early boot: no link was ever wanted.
    expect(selectBlockingState(make({ hosted: 'unknown' }))).toBe('ok');
    // A store hydrated before the channel existed reads the same way.
    expect(
      selectBlockingState(make({ hosted: undefined as unknown as ConnectivityState['hosted'] }))
    ).toBe('ok');
  });

  it('ranks the hosted link below every other channel', () => {
    expect(selectBlockingState(make({ backend: 'disconnected', hosted: 'reconnecting' }))).toBe(
      'backend-only'
    );
    expect(selectBlockingState(make({ core: 'unreachable', hosted: 'reconnecting' }))).toBe(
      'core-unreachable'
    );
    expect(selectBlockingState(make({ internet: 'offline', hosted: 'reconnecting' }))).toBe(
      'internet-offline'
    );
  });
});

describe('isHostedDegraded', () => {
  it('is true only for a link that is down and being retried', () => {
    expect(isHostedDegraded('connecting')).toBe(true);
    expect(isHostedDegraded('reconnecting')).toBe(true);
    expect(isHostedDegraded('error')).toBe(true);
    expect(isHostedDegraded('connected')).toBe(false);
    expect(isHostedDegraded('unknown')).toBe(false);
    expect(isHostedDegraded(undefined)).toBe(false);
  });
});
