/**
 * Regression tests for the active-user boot decision that gates #4545.
 *
 * The end-to-end loop the fix prevents (see `resolveActiveUserBootstrap`
 * docblock):
 *   1. User picks Cloud/Remote core → boot succeeds
 *   2. `CoreStateProvider` fetches the remote snapshot → nextIdentity = REMOTE user
 *   3. Boot primes seed from local `active_user.toml` → seedUserId = OLD LOCAL user
 *   4. `seedUserId !== nextIdentity` → `handleIdentityFlip` → `restartApp`
 *   5. Repeat forever
 *
 * The fix short-circuits step 3 whenever `coreMode === 'cloud'` (or the
 * window is a standalone native mascot/notch webview with no Tauri IPC).
 */
import { describe, expect, test, vi } from 'vitest';

import {
  consumeIdentityFlipSeed,
  markIdentityFlipSeed,
  resolveActiveUserBootstrap,
  shouldSkipLocalActiveUserRead,
} from '../bootstrapActiveUser';

describe('shouldSkipLocalActiveUserRead', () => {
  test('cloud mode skips the local read', () => {
    expect(
      shouldSkipLocalActiveUserRead({ isStandaloneNativeWindow: false, coreMode: 'cloud' })
    ).toBe(true);
  });

  test('local mode reads through', () => {
    expect(
      shouldSkipLocalActiveUserRead({ isStandaloneNativeWindow: false, coreMode: 'local' })
    ).toBe(false);
  });

  test('unset mode reads through (fresh install falls back to local file → likely null → prime(null))', () => {
    expect(shouldSkipLocalActiveUserRead({ isStandaloneNativeWindow: false, coreMode: null })).toBe(
      false
    );
  });

  test('standalone native window skips regardless of core mode', () => {
    expect(
      shouldSkipLocalActiveUserRead({ isStandaloneNativeWindow: true, coreMode: 'local' })
    ).toBe(true);
    expect(
      shouldSkipLocalActiveUserRead({ isStandaloneNativeWindow: true, coreMode: 'cloud' })
    ).toBe(true);
  });
});

describe('resolveActiveUserBootstrap', () => {
  test('cloud mode resolves null WITHOUT calling getActiveUserIdFromCore (#4545)', async () => {
    const getActiveUserIdFromCore = vi.fn().mockResolvedValue('stale-local-user');
    const result = await resolveActiveUserBootstrap({
      isStandaloneNativeWindow: false,
      coreMode: 'cloud',
      getActiveUserIdFromCore,
    });
    expect(result).toBeNull();
    expect(getActiveUserIdFromCore).not.toHaveBeenCalled();
  });

  test('local mode delegates to getActiveUserIdFromCore', async () => {
    const getActiveUserIdFromCore = vi.fn().mockResolvedValue('user-A');
    const result = await resolveActiveUserBootstrap({
      isStandaloneNativeWindow: false,
      coreMode: 'local',
      getActiveUserIdFromCore,
    });
    expect(result).toBe('user-A');
    expect(getActiveUserIdFromCore).toHaveBeenCalledTimes(1);
  });

  test('unset mode delegates to getActiveUserIdFromCore (pre-picker cold boot)', async () => {
    const getActiveUserIdFromCore = vi.fn().mockResolvedValue(null);
    const result = await resolveActiveUserBootstrap({
      isStandaloneNativeWindow: false,
      coreMode: null,
      getActiveUserIdFromCore,
    });
    expect(result).toBeNull();
    expect(getActiveUserIdFromCore).toHaveBeenCalledTimes(1);
  });

  test('standalone native window resolves null WITHOUT calling the IPC', async () => {
    const getActiveUserIdFromCore = vi.fn().mockResolvedValue('should-not-be-read');
    const result = await resolveActiveUserBootstrap({
      isStandaloneNativeWindow: true,
      coreMode: 'local',
      getActiveUserIdFromCore,
    });
    expect(result).toBeNull();
    expect(getActiveUserIdFromCore).not.toHaveBeenCalled();
  });
});

describe('gateway mode is a remote core, not a local one', () => {
  // Regression: `getStoredCoreMode` did not recognise `gateway` and returned
  // null, which reads as "local" everywhere this is consulted. Priming the
  // active user from the local `active_user.toml` then overwrites the seed
  // `handleIdentityFlip` wrote, which is the #4545 restart loop — reintroduced
  // for exactly the users the gateway feature exists for.
  it('skips the local active-user read, the same as cloud', () => {
    expect(
      shouldSkipLocalActiveUserRead({ isStandaloneNativeWindow: false, coreMode: 'gateway' })
    ).toBe(true);
  });

  it('still reads locally for local mode', () => {
    expect(
      shouldSkipLocalActiveUserRead({ isStandaloneNativeWindow: false, coreMode: 'local' })
    ).toBe(false);
  });

  it('resolves null rather than calling the core', async () => {
    const getActiveUserIdFromCore = vi.fn();

    await expect(
      resolveActiveUserBootstrap({
        isStandaloneNativeWindow: false,
        coreMode: 'gateway',
        getActiveUserIdFromCore,
      })
    ).resolves.toBeNull();
    expect(getActiveUserIdFromCore).not.toHaveBeenCalled();
  });
});

describe('identity-flip seed marker (#4545, local-core variant)', () => {
  // The loop this guards: sign in with TinyHumans after using "Continue
  // Locally". `handleIdentityFlip` corrects the seed and restarts, but a LOCAL
  // core re-primes from `active_user.toml`, which still names the local user.
  // The next refresh sees the same mismatch and restarts again, forever.
  it('prefers a pending flip seed over the core file, and consumes it once', async () => {
    const getActiveUserIdFromCore = vi.fn().mockResolvedValue('local-stale-user');
    const consumeIdentityFlipSeed = vi
      .fn()
      .mockReturnValueOnce('tinyhumans-user')
      .mockReturnValue(null);

    const first = await resolveActiveUserBootstrap({
      isStandaloneNativeWindow: false,
      coreMode: 'local',
      getActiveUserIdFromCore,
      consumeIdentityFlipSeed,
    });
    expect(first).toBe('tinyhumans-user');
    // The stale file is never consulted while a flip is pending.
    expect(getActiveUserIdFromCore).not.toHaveBeenCalled();

    // Second boot: the marker is spent, so the normal source takes over.
    const second = await resolveActiveUserBootstrap({
      isStandaloneNativeWindow: false,
      coreMode: 'local',
      getActiveUserIdFromCore,
      consumeIdentityFlipSeed,
    });
    expect(second).toBe('local-stale-user');
    expect(getActiveUserIdFromCore).toHaveBeenCalledTimes(1);
  });

  it('leaves cloud mode on its existing path when no flip is pending', async () => {
    const getActiveUserIdFromCore = vi.fn().mockResolvedValue('should-not-be-read');
    const resolved = await resolveActiveUserBootstrap({
      isStandaloneNativeWindow: false,
      coreMode: 'cloud',
      getActiveUserIdFromCore,
      consumeIdentityFlipSeed: () => null,
    });
    expect(resolved).toBeNull();
    expect(getActiveUserIdFromCore).not.toHaveBeenCalled();
  });
});

describe('consumeIdentityFlipSeed storage failures', () => {
  // Private windows, cleared site data and quota errors all make localStorage
  // throw. The loop guard is best-effort: it must degrade to "no flip pending"
  // rather than take the boot path down with it.
  it('returns null when reading localStorage throws', () => {
    const getItem = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('SecurityError');
    });
    try {
      expect(consumeIdentityFlipSeed()).toBeNull();
    } finally {
      getItem.mockRestore();
    }
  });

  it('round-trips a marker and clears it so the next boot sees nothing', () => {
    markIdentityFlipSeed('user-abc');
    expect(consumeIdentityFlipSeed()).toBe('user-abc');
    expect(consumeIdentityFlipSeed()).toBeNull();
  });

  it('treats a blank marker as no flip pending', () => {
    markIdentityFlipSeed('   ');
    expect(consumeIdentityFlipSeed()).toBeNull();
  });
});
