import type { CustomStepKey } from './OnboardingContext';

/** Ordered list of custom-wizard steps. Index drives the step counter UI and
 *  the back/continue navigation.
 *
 *  Cut from six steps to three. Voice, OAuth and embeddings were a forced
 *  linear tour of Settings standing between a self-hosting user and their
 *  first message; all three remain configurable from Settings, and their step
 *  page files stay on disk (same convention as the retired Gmail/Composio
 *  steps — see the comment in `Onboarding.tsx`). */
export const CUSTOM_WIZARD_STEPS: CustomStepKey[] = ['inference', 'search', 'vault'];

export const CUSTOM_WIZARD_ROUTES: Record<CustomStepKey, string> = {
  inference: '/onboarding/custom/inference',
  voice: '/onboarding/custom/voice',
  oauth: '/onboarding/custom/oauth',
  search: '/onboarding/custom/search',
  embeddings: '/onboarding/custom/embeddings',
  memory: '/onboarding/custom/memory',
  vault: '/onboarding/custom/vault',
};

/**
 * The stepper label for each step key.
 *
 * This map exists so the stepper cannot disagree with `CUSTOM_WIZARD_STEPS`
 * about which label belongs to which step. It replaces a hand-ordered label
 * array that was sliced to the step count: when the wizard was cut to three
 * steps that slice silently rendered "Inference / Voice / OAuth" — the first
 * three labels of the old six-step order — instead of the three steps
 * actually on screen. Keying labels off the step itself removes the ordering
 * as something anyone has to keep in sync.
 */
export const STEP_LABEL_KEYS: Record<CustomStepKey, string> = {
  inference: 'onboarding.custom.stepperInference',
  voice: 'onboarding.custom.stepperVoice',
  oauth: 'onboarding.custom.stepperOAuth',
  search: 'onboarding.custom.stepperSearch',
  embeddings: 'onboarding.custom.stepperEmbeddings',
  memory: 'onboarding.custom.stepperMemory',
  vault: 'onboarding.custom.stepperVault',
};
