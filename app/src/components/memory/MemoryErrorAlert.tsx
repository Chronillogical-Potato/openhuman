/**
 * How a failed memory action is shown. An exhausted credit balance
 * (`INSUFFICIENT_CREDITS`) is a prompt to top up, not an error: it explains
 * that nothing stored was lost and links to billing. Every other failure keeps
 * the destructive alert.
 */
import type { ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';

import { useT } from '../../lib/i18n/I18nContext';
import { isOutOfCreditsMessage } from '../../services/api/memoryApi';
import { Alert, AlertDescription, AlertTitle, Button } from '../ui';

/** Where the app sends every "top up" action (`open_billing` in useAppNotices). */
const BILLING_ROUTE = '/settings/account';

interface MemoryCreditsPromptProps {
  /** The out-of-credits explanation. */
  message: string;
  className?: string;
  'data-testid'?: string;
}

/** "Out of credits: top up", with a button to billing. */
export function MemoryCreditsPrompt({
  message,
  className,
  'data-testid': testId,
}: MemoryCreditsPromptProps) {
  const { t } = useT();
  const navigate = useNavigate();
  return (
    <Alert variant="warning" className={className} data-testid={testId} data-kind="out-of-credits">
      <AlertTitle>{t('memory.outOfCredits.title')}</AlertTitle>
      <AlertDescription className="flex flex-wrap items-center gap-2">
        <span>{message}</span>
        <Button
          type="button"
          size="xs"
          variant="primary"
          data-testid="memory-top-up"
          onClick={() => navigate(BILLING_ROUTE)}>
          {t('memory.outOfCredits.action')}
        </Button>
      </AlertDescription>
    </Alert>
  );
}

interface MemoryErrorAlertProps {
  /** The message `memoryErrorMessage(err, t)` produced. */
  message: string;
  className?: string;
  'data-testid'?: string;
  /** Replaces the message inside the error alert (e.g. message + retry). */
  children?: ReactNode;
}

/** The credits prompt for an exhausted balance, else the error alert. */
export default function MemoryErrorAlert({
  message,
  className,
  'data-testid': testId,
  children,
}: MemoryErrorAlertProps) {
  const { t } = useT();
  if (isOutOfCreditsMessage(message, t)) {
    return <MemoryCreditsPrompt message={message} className={className} data-testid={testId} />;
  }
  return (
    <Alert variant="destructive" className={className} data-testid={testId}>
      <AlertDescription>{children ?? message}</AlertDescription>
    </Alert>
  );
}
