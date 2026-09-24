import { useEffect, useRef } from 'react';

import { DEV_SKIP_ONBOARDING } from '../utils/config';

interface DevSkipOnboardingInput {
  isBootstrapping: boolean;
  sessionToken: string | null | undefined;
  onboardingCompleted: boolean | undefined;
  setOnboardingCompletedFlag: (value: boolean) => Promise<void>;
}

/**
 * Dev-only auto-skip (`VITE_DEV_SKIP_ONBOARDING`, set by
 * `scripts/run-dev-web.sh`): once a signed-in user's core reports onboarding
 * incomplete, record completion in the core — once per mount — rather than
 * only hiding the stepper, so state keyed off the core flag agrees with the UI.
 */
export function useDevSkipOnboarding(
  {
    isBootstrapping,
    sessionToken,
    onboardingCompleted,
    setOnboardingCompletedFlag,
  }: DevSkipOnboardingInput,
  enabled: boolean = DEV_SKIP_ONBOARDING
): void {
  const requestedRef = useRef(false);
  useEffect(() => {
    if (!enabled || requestedRef.current) return;
    if (isBootstrapping || !sessionToken || onboardingCompleted) return;
    requestedRef.current = true;
    console.debug('[onboarding-gate] dev skip: marking onboarding complete');
    void setOnboardingCompletedFlag(true).catch(err =>
      console.warn('[onboarding-gate] dev skip: could not mark onboarding complete', err)
    );
  }, [enabled, isBootstrapping, sessionToken, onboardingCompleted, setOnboardingCompletedFlag]);
}
