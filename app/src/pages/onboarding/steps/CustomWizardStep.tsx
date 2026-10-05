import { type ReactNode, useState } from 'react';

import { Button, Card } from '../../../components/ui';
import { useT } from '../../../lib/i18n/I18nContext';
import OnboardingNextButton from '../components/OnboardingNextButton';
import WizardStepper from '../components/WizardStepper';
import { CUSTOM_WIZARD_STEPS, STEP_LABEL_KEYS } from '../customWizardSteps';

interface CustomWizardStepProps {
  stepIndex: number;
  stepCount: number;
  title: string;
  subtitle: string;
  /**
   * The step's configuration surface, rendered directly.
   *
   * There used to be a Default/Configure fork above this. It asked the same
   * question the runtime choice already answered — a user reaches these steps
   * precisely by saying they want to configure things themselves — and
   * "Default" branched to nothing: no code ever read the recorded choice
   * except the analytics tag. The step is the configuration now.
   */
  configureContent?: ReactNode;
  onBack: () => void;
  onContinue: () => void | Promise<void>;
  /** Continue label override (used for the final "Finish setup" step). */
  continueLabel?: string;
  /** Disable the continue button (e.g. while an inline save is in flight). */
  continueDisabled?: boolean;
  /** Replace the continue button text with a busy label while loading. */
  continueLoading?: boolean;
  continueLoadingLabel?: string;
  testId?: string;
  /** Explains a blocked Continue (e.g. the panel has unsaved edits). */
  continueHint?: string;
  /** Rendered beneath the footer — the search step's "Skip for now". */
  secondaryAction?: ReactNode;
}

const CustomWizardStep = ({
  stepIndex,
  stepCount,
  title,
  subtitle,
  configureContent,
  onBack,
  onContinue,
  continueLabel,
  continueDisabled,
  continueLoading,
  continueLoadingLabel,
  testId,
  continueHint,
  secondaryAction,
}: CustomWizardStepProps) => {
  const { t } = useT();
  const [isContinuing, setIsContinuing] = useState(false);

  const handleContinue = async () => {
    if (isContinuing || continueDisabled) return;
    try {
      setIsContinuing(true);
      await onContinue();
    } finally {
      setIsContinuing(false);
    }
  };

  // Derived from the step list itself rather than a parallel hand-ordered
  // array, which could be — and was — sliced into labels belonging to steps
  // that are no longer rendered. See STEP_LABEL_KEYS.
  const stepperLabels = CUSTOM_WIZARD_STEPS.map(key => t(STEP_LABEL_KEYS[key]));

  const rootTestId = testId ?? 'onboarding-custom-wizard-step';

  return (
    <Card
      padded
      divided={false}
      data-testid={rootTestId}
      className="animate-fade-up p-6 shadow-soft sm:p-8">
      <WizardStepper labels={stepperLabels} activeIndex={stepIndex} />

      <p
        className="mt-8 text-[11px] font-medium uppercase tracking-wide text-content-faint"
        data-testid="onboarding-step-counter">
        {t('onboarding.custom.stepCounter')
          .replace('{n}', String(stepIndex + 1))
          .replace('{total}', String(stepCount))}
      </p>
      <h1 className="mt-1 text-2xl font-title text-content leading-tight">{title}</h1>
      <p className="mt-2 text-sm text-content-muted leading-relaxed">{subtitle}</p>

      {/* No wrapper card here: the embedded panels are themselves `Card`s, and
          nesting one in another produced a card-in-a-card. */}
      {configureContent ? <div className="mt-6">{configureContent}</div> : null}

      {/* Back and Continue carry equal width. Continue used to sit in a
          `flex-1` wrapper beside an intrinsically-sized Back, so the primary
          action ran the width of the card while Back shrank to its label —
          a hierarchy nobody chose. */}
      <div className="mt-8 flex items-stretch gap-3">
        {/* `size="lg"` matches OnboardingNextButton, which is also lg. Left at
            the default md, Back rendered h-9 next to a h-11 Continue — equal
            width but visibly unequal height. */}
        <Button variant="secondary" size="lg" onClick={onBack} className="flex-1 basis-0">
          {t('onboarding.custom.back')}
        </Button>
        <div className="flex-1 basis-0">
          <OnboardingNextButton
            label={continueLabel ?? t('onboarding.custom.continue')}
            onClick={() => void handleContinue()}
            disabled={continueDisabled || isContinuing}
            loading={continueLoading || isContinuing}
            loadingLabel={continueLoadingLabel}
          />
        </div>
      </div>

      {continueHint ? (
        <p
          className="mt-3 text-center text-xs text-amber-700 dark:text-amber-300"
          data-testid="onboarding-continue-hint">
          {continueHint}
        </p>
      ) : null}

      {secondaryAction ? <div className="mt-3 flex justify-center">{secondaryAction}</div> : null}
    </Card>
  );
};

export default CustomWizardStep;
