import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import MemoryEngineSetup from '../../../components/memory/MemoryEngineSetup';
import { Alert, AlertDescription } from '../../../components/ui';
import { useIsLocalSession } from '../../../hooks/useLocalSession';
import { useT } from '../../../lib/i18n/I18nContext';
import { trackEvent } from '../../../services/analytics';
import { CUSTOM_WIZARD_ROUTES, CUSTOM_WIZARD_STEPS } from '../customWizardSteps';
import { type CustomStepChoice, useOnboardingContext } from '../OnboardingContext';
import CustomWizardStep from '../steps/CustomWizardStep';

const STEP_KEY = 'vault' as const;

export default function VaultSetupStep() {
  const { t } = useT();
  const navigate = useNavigate();
  const { draft, setDraft, completeAndExit } = useOnboardingContext();
  const stepIndex = CUSTOM_WIZARD_STEPS.indexOf(STEP_KEY);
  const isLocalSession = useIsLocalSession();

  const initialChoice = isLocalSession ? 'configure' : (draft.customChoices?.[STEP_KEY] ?? null);
  const [choice, setChoice] = useState<CustomStepChoice | null>(initialChoice);
  const [exitError, setExitError] = useState<string | null>(null);

  // A local session has no managed option to choose between, so the step is
  // pinned to `configure`. This runs as an effect, matching every other step
  // (`CustomWizardConfigPage`); it used to be a ref-guarded setState during
  // render, which is the same intent written a second, riskier way.
  useEffect(() => {
    if (!isLocalSession) return;
    setChoice('configure');
    setDraft(prev => ({
      ...prev,
      customChoices: { ...prev.customChoices, [STEP_KEY]: 'configure' },
    }));
  }, [isLocalSession, setDraft]);

  const persistChoice = useCallback(
    (next: CustomStepChoice) => {
      setChoice(next);
      setDraft(prev => ({ ...prev, customChoices: { ...prev.customChoices, [STEP_KEY]: next } }));
    },
    [setDraft]
  );

  const configureContent = useMemo(() => <MemoryEngineSetup />, []);

  return (
    <>
      <CustomWizardStep
        testId="onboarding-custom-vault-step"
        stepIndex={stepIndex}
        stepCount={CUSTOM_WIZARD_STEPS.length}
        title={t('onboarding.custom.vault.title')}
        subtitle={t('onboarding.custom.vault.subtitle')}
        defaultDescription={t('onboarding.custom.vault.defaultDesc')}
        configureDescription={t('onboarding.custom.vault.configureDesc')}
        configureContent={configureContent}
        defaultDisabled={isLocalSession}
        defaultDisabledReason={
          isLocalSession ? t('onboarding.custom.vault.localDisabledReason') : undefined
        }
        hideChoiceCards={isLocalSession}
        choice={choice}
        onChoiceChange={persistChoice}
        onBack={() => navigate(CUSTOM_WIZARD_ROUTES[CUSTOM_WIZARD_STEPS[stepIndex - 1]])}
        onContinue={async () => {
          setExitError(null);
          trackEvent('onboarding_step_complete', {
            step_name: 'custom_vault',
            choice: choice ?? 'default',
          });
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
