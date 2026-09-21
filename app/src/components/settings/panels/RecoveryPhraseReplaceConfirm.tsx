import { useT } from '../../../lib/i18n/I18nContext';
import { WarningIcon } from '../../ui';
import { Alert } from '../../ui/Alert';
import Button from '../../ui/Button';

export interface RecoveryPhraseReplaceConfirmProps {
  onConfirm: () => void;
  onCancel: () => void;
  confirmText: string;
  warningText: string;
}

// Inline "Replace wallet" confirmation gate. Deliberately NOT a modal
// (Dialog/AlertDialog/ConfirmDialog): the panel already renders this as a
// dedicated mode, and the e2e/unit specs locate its copy and buttons via
// `screen.getByText` against the document — wrapping it in a portal-based
// dialog would not change behavior but adds risk for no test-visible benefit.
const RecoveryPhraseReplaceConfirm = ({
  onConfirm,
  onCancel,
  confirmText,
  warningText,
}: RecoveryPhraseReplaceConfirmProps) => {
  const { t } = useT();

  return (
    <div className="space-y-5">
      <Alert variant="destructive" className="border-0 bg-destructive/10 p-4">
        <div className="flex items-start gap-3 text-destructive">
          <WarningIcon className="w-5 h-5 shrink-0 mt-0.5" />
          <p className="text-sm leading-relaxed">{warningText}</p>
        </div>
      </Alert>

      <Button
        type="button"
        variant="primary"
        tone="danger"
        size="md"
        onClick={onConfirm}
        className="w-full">
        {confirmText}
      </Button>

      <Button type="button" variant="tertiary" onClick={onCancel} className="w-full">
        {t('common.cancel')}
      </Button>
    </div>
  );
};

export default RecoveryPhraseReplaceConfirm;
