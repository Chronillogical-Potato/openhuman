import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { Alert, AlertDescription, Button } from '../../../components/ui';
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
  const [retrying, setRetrying] = useState(false);
  const handled = useRef(false);

  const finishManaged = useCallback(async () => {
    setExitError(null);
    setRetrying(true);
    try {
      await completeAndExit();
    } catch (err) {
      console.error('[onboarding:welcome] completeAndExit failed', err);
      setExitError(err instanceof Error ? err.message : String(err));
    } finally {
      setRetrying(false);
    }
  }, [completeAndExit]);

  useEffect(() => {
    if (handled.current) return;
    handled.current = true;
    trackEvent('onboarding_start');

    if (isLocalSession) {
      navigate('/onboarding/custom/inference', { replace: true });
      return;
    }

    trackEvent('onboarding_step_complete', { step_name: 'managed' });
    void finishManaged();
  }, [isLocalSession, navigate, finishManaged]);

  if (!exitError) return null;

  // The flag write is the only thing between a managed user and chat, so a
  // failure needs a control. Resetting a ref does not re-render, so the effect
  // alone could never retry -- this page would have been a dead end.
  return (
    <Alert variant="destructive" data-testid="onboarding-welcome-exit-error">
      <AlertDescription>
        <p>{t('onboarding.custom.vault.exitError')}</p>
        <Button
          variant="secondary"
          size="sm"
          className="mt-3"
          disabled={retrying}
          onClick={() => void finishManaged()}
          data-testid="onboarding-welcome-retry">
          {t('welcome.handoff.retry')}
        </Button>
      </AlertDescription>
    </Alert>
  );
};

export default WelcomePage;
