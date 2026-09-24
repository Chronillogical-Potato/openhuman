/**
 * The composer's model trigger, backed directly by OpenHuman's shared custom
 * provider/model picker. The compact trigger stays in the composer; clicking
 * it opens the same modal UI used elsewhere instead of an intermediate
 * assistant-ui settings popover.
 */
import { ChevronDownIcon } from 'lucide-react';
import { useMemo, useState } from 'react';

import {
  displayValue,
  NO_LOCAL_MODELS,
  selectionFromValue,
  selectionValue,
  useModelPickerProviders,
} from '../../../components/chat/ModelQualityPill';
import { ProviderModelPickerDialog } from '../../../components/settings/panels/ai/ProviderModelPickerDialog';
import { useT } from '../../../lib/i18n/I18nContext';

export function ChatSettingsPanel({
  model,
  onModelChange,
}: {
  model: string | null;
  /** `null` clears the composer's pin back to the managed default. */
  onModelChange?: (value: string | null, contextWindow?: number | null) => void;
}) {
  const { t } = useT();
  const [open, setOpen] = useState(false);
  const { providers, loading } = useModelPickerProviders(open);
  const initialSelection = useMemo(() => selectionFromValue(model), [model]);

  return (
    <>
      <button
        type="button"
        data-testid="composer-chat-settings"
        data-analytics-id="chat-settings-panel"
        aria-label={t('composer.modelSelector')}
        title={t('composer.modelSelector')}
        disabled={!onModelChange || loading}
        onClick={() => setOpen(true)}
        className="flex h-7 min-w-0 items-center rounded-md px-2 text-xs text-content-muted transition-colors hover:bg-surface-hover hover:text-content disabled:opacity-50">
        <span className="min-w-0 truncate font-medium">
          {loading ? t('composer.settings.loadingModels') : displayValue(model)}
        </span>
        <ChevronDownIcon className="ml-1 size-3.5 shrink-0 opacity-50" aria-hidden />
      </button>

      {open && !loading && (
        <ProviderModelPickerDialog
          cloudProviders={providers}
          localModels={NO_LOCAL_MODELS}
          ollamaRunning={false}
          claudeCodeEnabled={false}
          initial={initialSelection}
          onClose={() => setOpen(false)}
          onSelect={selection => {
            onModelChange?.(selectionValue(selection), selection.contextWindow);
            setOpen(false);
          }}
        />
      )}
    </>
  );
}

export default ChatSettingsPanel;
