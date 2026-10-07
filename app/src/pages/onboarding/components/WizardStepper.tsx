import { Stepper } from '../../../components/ui';
import { useT } from '../../../lib/i18n/I18nContext';

interface WizardStepperProps {
  /** Ordered labels for each step in the wizard. */
  labels: string[];
  /** Zero-based index of the step that is currently active. */
  activeIndex: number;
}

/**
 * Thin wrapper over the shared `Stepper` primitive that supplies the
 * onboarding copy and the E2E test id.
 */
const WizardStepper = ({ labels, activeIndex }: WizardStepperProps) => {
  const { t } = useT();
  return (
    <Stepper
      labels={labels}
      activeIndex={activeIndex}
      ariaLabel={t('onboarding.custom.progressAriaLabel')}
      data-testid="onboarding-wizard-stepper"
    />
  );
};

export default WizardStepper;
