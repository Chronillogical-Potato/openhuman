import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { Alert, AlertDescription } from '../../../components/ui';
import { useIsLocalSession } from '../../../hooks/useLocalSession';
import { useT } from '../../../lib/i18n/I18nContext';
import { trackEvent } from '../../../services/analytics';
import { useOnboardingContext } from '../OnboardingContext';

/**
 * The onboarding entry point, which now routes rather than asks.
 *
 * The identity question moved to the Welcome screen (`pages/Welcome.tsx`),
 * where it is asked once as two cards. By the time anyone reaches here it is
 * already answered, so this page only honours the answer:
 *
 *   TinyHumans session -> nothing is left to configure. TinyHumans supplies
 *     inference, voice, search, memory and embeddings, so onboarding is marked
 *     complete and the user lands in chat.
 *   local session      -> the three self-hosted steps.
 *
 * The old welcome -> runtime-choice pair asked the same question a second
 * time, three screens after the first.
 */
const WelcomePage = () => {
  const { t } = useT();
  const navigate = useNavigate();
  const isLocalSession = useIsLocalSession();
  const { completeAndExit } = useOnboardingContext();
  const [exitError, setExitError] = useState<string | null>(null);
  const handled = useRef(false);

  useEffect(() => {
    if (handled.current) return;
    handled.current = true;
    trackEvent('onboarding_start');

    if (isLocalSession) {
      navigate('/onboarding/custom/inference', { replace: true });
      return;
    }

    trackEvent('onboarding_step_complete', { step_name: 'managed' });
    void completeAndExit().catch(err => {
      // Leave the user here with a retry rather than on a blank page: the
      // flag write is the only thing between them and chat.
      handled.current = false;
      console.error('[onboarding:welcome] completeAndExit failed', err);
      setExitError(err instanceof Error ? err.message : String(err));
    });
  }, [isLocalSession, navigate, completeAndExit]);

  if (!exitError) return null;

  return (
    <Alert variant="destructive" data-testid="onboarding-welcome-exit-error">
      <AlertDescription>{t('onboarding.custom.vault.exitError')}</AlertDescription>
    </Alert>
  );
};

export default WelcomePage;
