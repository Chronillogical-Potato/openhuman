import { useState } from 'react';

import { Alert, AlertDescription, Button, CloseIcon } from '../../../components/ui';
import { useT } from '../../../lib/i18n/I18nContext';
import { DISCORD_INVITE_URL } from '../../../utils/links';

const DISMISSED_KEY = 'openhuman_beta_banner_dismissed';

const BetaBanner = () => {
  const { t } = useT();
  const [visible, setVisible] = useState(() => {
    try {
      return localStorage.getItem(DISMISSED_KEY) !== 'true';
    } catch {
      return true;
    }
  });

  if (!visible) return null;

  const handleDismiss = () => {
    try {
      localStorage.setItem(DISMISSED_KEY, 'true');
    } catch {
      // localStorage unavailable — dismiss for this session only
    }
    setVisible(false);
  };

  return (
    <Alert variant="warning" role={undefined} className="mb-4 items-start">
      <AlertDescription className="flex-1 text-xs">
        {t('misc.beta')}{' '}
        <a
          href={DISCORD_INVITE_URL}
          target="_blank"
          rel="noopener noreferrer"
          className="font-medium underline underline-offset-2 hover:opacity-80">
          {t('misc.betaFeedback')}
        </a>
      </AlertDescription>
      <Button
        variant="tertiary"
        size="xs"
        iconOnly
        aria-label={t('common.dismiss')}
        onClick={handleDismiss}
        className="shrink-0">
        <CloseIcon className="h-3.5 w-3.5" aria-hidden="true" />
      </Button>
    </Alert>
  );
};

export default BetaBanner;
