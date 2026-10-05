import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import MemoryEngineSetup from '../../../components/memory/MemoryEngineSetup';
import { Alert, AlertDescription } from '../../../components/ui';
import { useT } from '../../../lib/i18n/I18nContext';
import { trackEvent } from '../../../services/analytics';
import { CUSTOM_WIZARD_ROUTES, CUSTOM_WIZARD_STEPS } from '../customWizardSteps';
import { useOnboardingContext } from '../OnboardingContext';
import CustomWizardStep from '../steps/CustomWizardStep';

const STEP_KEY = 'vault' as const;

/**
 * Step 3 — where OpenHuman keeps what it learns.
 *
 * The Default/Configure fork that used to sit above this is gone: it asked
 * again what the runtime choice already answered, and "Default" branched to
 * nothing. The engine picker is the step.
 */
export default function VaultSetupStep() {
  const { t } = useT();
  const navigate = useNavigate();
  const { completeAndExit } = useOnboardingContext();
  const stepIndex = CUSTOM_WIZARD_STEPS.indexOf(STEP_KEY);
  const [exitError, setExitError] = useState<string | null>(null);

  const configureContent = useMemo(() => <MemoryEngineSetup />, []);

  return (
    <>
      <CustomWizardStep
        testId="onboarding-custom-vault-step"
        stepIndex={stepIndex}
        stepCount={CUSTOM_WIZARD_STEPS.length}
        title={t('onboarding.custom.vault.title')}
        subtitle={t('onboarding.custom.vault.subtitle')}
        configureContent={configureContent}
        onBack={() => navigate(CUSTOM_WIZARD_ROUTES[CUSTOM_WIZARD_STEPS[stepIndex - 1]])}
        onContinue={async () => {
          setExitError(null);
          trackEvent('onboarding_step_complete', { step_name: 'custom_vault' });
          try {
            await completeAndExit();
          } catch (err) {
            const message = err instanceof Error ? err.message : String(err);
            console.error('[onboarding:custom-vault] completeAndExit failed', err);
            setExitError(message);
          }
        }}
        continueLabel={t('onboarding.custom.finish')}
      />
      {exitError ? (
        <Alert variant="destructive" className="mt-3" data-testid="onboarding-vault-exit-error">
          <AlertDescription>{t('onboarding.custom.vault.exitError')}</AlertDescription>
        </Alert>
      ) : null}
    </>
  );
}
