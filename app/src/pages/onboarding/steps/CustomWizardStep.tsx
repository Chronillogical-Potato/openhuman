import { type ReactNode, useState } from 'react';

import { Button, Card, Tile, TileGrid } from '../../../components/ui';
import { useT } from '../../../lib/i18n/I18nContext';
import OnboardingNextButton from '../components/OnboardingNextButton';
import WizardStepper from '../components/WizardStepper';
import { CUSTOM_WIZARD_STEPS, STEP_LABEL_KEYS } from '../customWizardSteps';
import type { CustomStepChoice } from '../OnboardingContext';

interface CustomWizardStepProps {
  stepIndex: number;
  stepCount: number;
  title: string;
  subtitle: string;
  defaultDescription: string;
  configureDescription: string;
  /** Inline content rendered below the choice cards when 'configure' is picked. */
  configureContent?: ReactNode;
  choice: CustomStepChoice | null;
  onChoiceChange: (choice: CustomStepChoice) => void;
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
  defaultDisabled?: boolean;
  defaultDisabledReason?: string;
  hideChoiceCards?: boolean;
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
  defaultDescription,
  configureDescription,
  configureContent,
  choice,
  onChoiceChange,
  onBack,
  onContinue,
  continueLabel,
  continueDisabled,
  continueLoading,
  continueLoadingLabel,
  testId,
  defaultDisabled = false,
  defaultDisabledReason,
  hideChoiceCards = false,
  continueHint,
  secondaryAction,
}: CustomWizardStepProps) => {
  const { t } = useT();
  const [isContinuing, setIsContinuing] = useState(false);

  const handleContinue = async () => {
    if (isContinuing || choice === null || continueDisabled) return;
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
  const choiceGroupName = `${rootTestId}-choice`;
  const defaultRadioId = `${rootTestId}-choice-default`;
  const configureRadioId = `${rootTestId}-choice-configure`;

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

      {!hideChoiceCards ? (
        <>
          {/* `Tile` carries the selected/muted states the two hand-rolled
              choice buttons used to reimplement with `border-2` and `!`
              important overrides. */}
          <TileGrid columns={2} className="mt-6">
            <Tile
              data-testid={`${rootTestId}-default`}
              htmlFor={defaultRadioId}
              title={t('onboarding.custom.defaultTitle')}
              description={defaultDescription || t('onboarding.custom.defaultSubtitle')}
              selected={choice === 'default'}
              muted={defaultDisabled}
              control={
                <input
                  id={defaultRadioId}
                  type="radio"
                  name={choiceGroupName}
                  className="mt-0.5 size-4 accent-primary-500"
                  checked={choice === 'default'}
                  disabled={defaultDisabled}
                  onChange={() => onChoiceChange('default')}
                />
              }
            />
            <Tile
              data-testid={`${rootTestId}-configure`}
              htmlFor={configureRadioId}
              title={t('onboarding.custom.configureTitle')}
              description={configureDescription || t('onboarding.custom.configureSubtitle')}
              selected={choice === 'configure'}
              control={
                <input
                  id={configureRadioId}
                  type="radio"
                  name={choiceGroupName}
                  className="mt-0.5 size-4 accent-primary-500"
                  checked={choice === 'configure'}
                  onChange={() => onChoiceChange('configure')}
                />
              }
            />
          </TileGrid>

          {defaultDisabled && defaultDisabledReason ? (
            <p className="mt-3 text-xs text-content-muted leading-relaxed">
              {defaultDisabledReason}
            </p>
          ) : null}
        </>
      ) : null}

      {/* No wrapper card here: the embedded panels are themselves `Card`s, and
          nesting one in another produced a card-in-a-card. */}
      {(choice === 'configure' || hideChoiceCards) && configureContent ? (
        <div className="mt-6">{configureContent}</div>
      ) : null}

      {/* Back and Continue carry equal width. Continue used to sit in a
          `flex-1` wrapper beside an intrinsically-sized Back, so the primary
          action ran the width of the card while Back shrank to its label —
          a hierarchy nobody chose. */}
      <div className="mt-8 flex items-stretch gap-3">
        <Button variant="secondary" onClick={onBack} className="flex-1 basis-0">
          {t('onboarding.custom.back')}
        </Button>
        <div className="flex-1 basis-0">
          <OnboardingNextButton
            label={continueLabel ?? t('onboarding.custom.continue')}
            onClick={() => void handleContinue()}
            disabled={choice === null || continueDisabled || isContinuing}
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
