import { useContext } from 'react';

import { getCoreStateSnapshot } from '../lib/coreState/store';
import { CoreStateContext } from '../providers/coreStateContext';
import { isLocalSessionToken } from '../utils/localSession';

/**
 * Session-shape helpers for surfaces that must tell a signed-in TinyHumans
 * user apart from a "Continue Locally" one.
 *
 * `isLocalSessionToken(snapshot.sessionToken)` is repeated across a dozen
 * components; these hooks give the two questions actually being asked a name,
 * and give the onboarding wizard one implementation instead of the two it had
 * (one driven by `useEffect`, one by a ref plus a setState during render).
 *
 * They read `CoreStateContext` directly rather than through `useCoreState()`,
 * which throws when no provider is mounted. Settings panels are rendered in
 * unit tests without one, so a throwing read would turn a visibility question
 * into a crash. When the context is absent the hooks fall back to the
 * module-level snapshot store, which is the same state the provider commits
 * to — it just does not re-render on change. That is acceptable here: a
 * session cannot change shape underneath a mounted panel, because an identity
 * flip restarts the app (`handleIdentityFlip` → `restartApp`).
 */
function readSessionToken(context: ReturnType<typeof useSessionContext>): string | null {
  return context ? context.snapshot.sessionToken : getCoreStateSnapshot().snapshot.sessionToken;
}

function useSessionContext() {
  return useContext(CoreStateContext);
}

/** True when the session is the offline "Continue Locally" one. */
export function useIsLocalSession(): boolean {
  const context = useSessionContext();
  return isLocalSessionToken(readSessionToken(context));
}

/**
 * True when OpenHuman-managed services (inference, embeddings, managed search
 * routes) are actually available — that is, when the session is a real
 * TinyHumans session. A local session has no account behind it and therefore
 * no managed anything, and a surface that claims otherwise is lying to the
 * user.
 */
export function useManagedInferenceAvailable(): boolean {
  const context = useSessionContext();
  const token = readSessionToken(context);
  return Boolean(token) && !isLocalSessionToken(token);
}
