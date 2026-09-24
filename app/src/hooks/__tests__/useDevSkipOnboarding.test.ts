import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { useDevSkipOnboarding } from '../useDevSkipOnboarding';

const base = { isBootstrapping: false, sessionToken: 'jwt', onboardingCompleted: false };

describe('useDevSkipOnboarding', () => {
  it('marks onboarding complete once for a signed-in, un-onboarded user', () => {
    const setFlag = vi.fn().mockResolvedValue(undefined);
    const { rerender } = renderHook(
      props => useDevSkipOnboarding({ ...props, setOnboardingCompletedFlag: setFlag }, true),
      { initialProps: base }
    );
    rerender({ ...base, sessionToken: 'jwt-2' });
    expect(setFlag).toHaveBeenCalledTimes(1);
    expect(setFlag).toHaveBeenCalledWith(true);
  });

  it('waits for bootstrap and a session, and leaves onboarded users alone', () => {
    const setFlag = vi.fn().mockResolvedValue(undefined);
    const { rerender } = renderHook(
      props => useDevSkipOnboarding({ ...props, setOnboardingCompletedFlag: setFlag }, true),
      { initialProps: { ...base, isBootstrapping: true } }
    );
    rerender({ ...base, sessionToken: '' });
    rerender({ ...base, onboardingCompleted: true });
    expect(setFlag).not.toHaveBeenCalled();
  });

  it('does nothing when the dev skip is off', () => {
    const setFlag = vi.fn().mockResolvedValue(undefined);
    renderHook(() => useDevSkipOnboarding({ ...base, setOnboardingCompletedFlag: setFlag }, false));
    expect(setFlag).not.toHaveBeenCalled();
  });

  it('swallows a failed write so the shell keeps rendering', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const setFlag = vi.fn().mockRejectedValue(new Error('CORE: down'));
    renderHook(() => useDevSkipOnboarding({ ...base, setOnboardingCompletedFlag: setFlag }, true));
    await vi.waitFor(() => expect(warn).toHaveBeenCalled());
    warn.mockRestore();
  });
});
