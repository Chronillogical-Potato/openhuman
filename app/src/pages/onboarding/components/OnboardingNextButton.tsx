import { Button } from '../../../components/ui';
import { useT } from '../../../lib/i18n/I18nContext';

interface OnboardingNextButtonProps {
  label?: string;
  onClick: () => void;
  disabled?: boolean;
  loading?: boolean;
  loadingLabel?: string;
}

const OnboardingNextButton = ({
  label,
  onClick,
  disabled = false,
  loading = false,
  loadingLabel,
}: OnboardingNextButtonProps) => {
  const { t } = useT();
  const effectiveLabel = label ?? t('common.continue');
  const effectiveLoadingLabel = loadingLabel ?? effectiveLabel;
  return (
    <Button
      variant="primary"
      size="lg"
      data-testid="onboarding-next-button"
      aria-label={effectiveLabel}
      aria-live="polite"
      aria-busy={loading}
      onClick={onClick}
      disabled={disabled || loading}
      className="w-full">
      {loading ? effectiveLoadingLabel : effectiveLabel}
    </Button>
  );
};

export default OnboardingNextButton;
