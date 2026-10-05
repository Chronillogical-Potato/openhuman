import { useNavigate } from 'react-router-dom';

import SearchPanel from '../../../components/settings/panels/SearchPanel';
import { Button } from '../../../components/ui';
import { useT } from '../../../lib/i18n/I18nContext';
import { trackEvent } from '../../../services/analytics';
import { CUSTOM_WIZARD_ROUTES, CUSTOM_WIZARD_STEPS } from '../customWizardSteps';
import CustomWizardConfigPage from './CustomWizardConfigPage';

const STEP_KEY = 'search' as const;

/**
 * Step 2 — web search.
 *
 * The only genuinely optional step of the three: without a key the agent still
 * works, it just cannot read the web. Forcing a key here is how people abandon
 * setup, so the step offers an explicit skip rather than a dead Continue.
 */
const CustomSearchPage = () => {
  const { t } = useT();
  const navigate = useNavigate();
  const stepIndex = CUSTOM_WIZARD_STEPS.indexOf(STEP_KEY);

  const handleSkip = () => {
    trackEvent('onboarding_step_complete', { step_name: `custom_${STEP_KEY}`, choice: 'skipped' });
    navigate(CUSTOM_WIZARD_ROUTES[CUSTOM_WIZARD_STEPS[stepIndex + 1]]);
  };

  return (
    <CustomWizardConfigPage
      stepKey={STEP_KEY}
      configureContent={<SearchPanel embedded hideTabChrome />}
      secondaryAction={
        <Button
          variant="tertiary"
          size="sm"
          onClick={handleSkip}
          analyticsId="onboarding-search-skip"
          data-testid="onboarding-search-skip">
          {t('onboarding.custom.search.skipForNow')}
        </Button>
      }
    />
  );
};

export default CustomSearchPage;
