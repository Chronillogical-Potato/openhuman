import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../../utils/config', async importOriginal => ({
  ...(await importOriginal<typeof import('../../../utils/config')>()),
  DEV_SKIP_ONBOARDING: true,
}));

describe('isWalkthroughPending with VITE_DEV_SKIP_ONBOARDING', () => {
  afterEach(() => localStorage.clear());

  it('never reports the tour as pending in a dev-skip session', async () => {
    const { isWalkthroughPending, setWalkthroughPending } = await import('../AppWalkthrough');
    setWalkthroughPending();
    expect(isWalkthroughPending(true)).toBe(false);
  });
});
