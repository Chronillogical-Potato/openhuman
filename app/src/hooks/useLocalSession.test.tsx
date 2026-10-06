import { renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import { getCoreStateSnapshot, setCoreStateSnapshot } from '../lib/coreState/store';
import { CoreStateContext } from '../providers/coreStateContext';
import { createLocalSessionToken } from '../utils/localSession';
import { useIsLocalSession, useManagedInferenceAvailable } from './useLocalSession';

const REMOTE_TOKEN = 'header.payload.signature';

function withContext(sessionToken: string | null) {
  const value = { snapshot: { sessionToken } } as never;
  return ({ children }: { children: ReactNode }) => (
    <CoreStateContext.Provider value={value}>{children}</CoreStateContext.Provider>
  );
}

function seedStore(sessionToken: string | null) {
  const current = getCoreStateSnapshot();
  setCoreStateSnapshot({ ...current, snapshot: { ...current.snapshot, sessionToken } });
}

describe('useLocalSession hooks with a CoreStateProvider', () => {
  it('treats a local token as a local session with no managed inference', () => {
    const wrapper = withContext(createLocalSessionToken());
    expect(renderHook(() => useIsLocalSession(), { wrapper }).result.current).toBe(true);
    expect(renderHook(() => useManagedInferenceAvailable(), { wrapper }).result.current).toBe(
      false
    );
  });

  it('treats a real session as non-local with managed inference available', () => {
    const wrapper = withContext(REMOTE_TOKEN);
    expect(renderHook(() => useIsLocalSession(), { wrapper }).result.current).toBe(false);
    expect(renderHook(() => useManagedInferenceAvailable(), { wrapper }).result.current).toBe(true);
  });

  it('signed out is neither local nor managed-capable', () => {
    const wrapper = withContext(null);
    expect(renderHook(() => useIsLocalSession(), { wrapper }).result.current).toBe(false);
    expect(renderHook(() => useManagedInferenceAvailable(), { wrapper }).result.current).toBe(
      false
    );
  });
});

describe('useLocalSession hooks without a CoreStateProvider', () => {
  const original = getCoreStateSnapshot();
  afterEach(() => setCoreStateSnapshot(original));

  it('does not throw and reads a local token from the snapshot store', () => {
    seedStore(createLocalSessionToken());
    expect(renderHook(() => useIsLocalSession()).result.current).toBe(true);
    expect(renderHook(() => useManagedInferenceAvailable()).result.current).toBe(false);
  });

  it('reads a real token from the snapshot store', () => {
    seedStore(REMOTE_TOKEN);
    expect(renderHook(() => useIsLocalSession()).result.current).toBe(false);
    expect(renderHook(() => useManagedInferenceAvailable()).result.current).toBe(true);
  });

  it('reports no managed inference when the store holds no session', () => {
    seedStore(null);
    expect(renderHook(() => useIsLocalSession()).result.current).toBe(false);
    expect(renderHook(() => useManagedInferenceAvailable()).result.current).toBe(false);
  });
});
