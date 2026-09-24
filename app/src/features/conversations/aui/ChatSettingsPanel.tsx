/**
 * The composer's chat-settings control: a trigger naming the active model,
 * with assistant-ui's settings-panel element in a popover behind it.
 *
 * Only settings a core config RPC persists are rendered:
 * - model — the composer route (`onModelChange`, which `Conversations`
 *   persists as `default_model` via `inference_update_model_settings`). The
 *   segmented row offers the managed default ("OpenHuman", clears the pin) and
 *   the current pick; any other model comes from the shared provider/model
 *   picker, whose sources load through `loadAISettings` exactly as the old
 *   composer pill's did (`useModelPickerProviders`).
 * - temperature — `config.default_temperature`, read from `config_get` when
 *   the popover opens and written through `inference_update_model_settings`.
 *   Hidden when the read fails (no core), rather than shown as a dead slider.
 *
 * The element's system-prompt field is never passed: no core config RPC
 * stores a chat system prompt, so the section stays omitted.
 */
import { SettingsPanel } from '@/components/assistant-ui/elements/settings-panel';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/assistant-ui/ui/popover';
import debug from 'debug';
import { ChevronDownIcon } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  displayValue,
  NO_LOCAL_MODELS,
  selectionFromValue,
  selectionValue,
  useModelPickerProviders,
} from '../../../components/chat/ModelQualityPill';
import { ProviderModelPickerDialog } from '../../../components/settings/panels/ai/ProviderModelPickerDialog';
import { Button } from '../../../components/ui';
import { useT } from '../../../lib/i18n/I18nContext';
import {
  openhumanGetConfig,
  openhumanUpdateModelSettings,
} from '../../../utils/tauriCommands/config';

const log = debug('openhuman:chat:settings-panel');

/** Label of the managed default — the same one the pill showed for no pin. */
const MANAGED_LABEL = displayValue(null);

/** A slider drag emits a value per step; persist only once it settles. */
const TEMPERATURE_SAVE_DELAY_MS = 400;

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
  const [pickerOpen, setPickerOpen] = useState(false);
  const { providers, loading } = useModelPickerProviders(pickerOpen);
  const [temperature, setTemperature] = useState<number | undefined>(undefined);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const current = displayValue(model);
  const models = useMemo(
    () => (current === MANAGED_LABEL ? [MANAGED_LABEL] : [MANAGED_LABEL, current]),
    [current]
  );
  const initialSelection = useMemo(() => selectionFromValue(model), [model]);

  useEffect(
    () => () => {
      if (saveTimer.current) clearTimeout(saveTimer.current);
    },
    []
  );

  const loadTemperature = useCallback(() => {
    log('config_get start');
    openhumanGetConfig().then(
      response => {
        const value = response.result.config.default_temperature;
        log('config_get ok has_temperature=%s', typeof value === 'number');
        setTemperature(typeof value === 'number' ? value : undefined);
      },
      (error: unknown) => {
        log('config_get failed, hiding temperature: %O', error);
        setTemperature(undefined);
      }
    );
  }, []);

  const handleOpenChange = useCallback(
    (next: boolean) => {
      setOpen(next);
      if (next) loadTemperature();
    },
    [loadTemperature]
  );

  const handleTemperatureChange = useCallback((next: number) => {
    setTemperature(next);
    if (saveTimer.current) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(() => {
      saveTimer.current = null;
      openhumanUpdateModelSettings({ default_temperature: next }).then(
        () => log('default_temperature persisted'),
        (error: unknown) => log('default_temperature persist failed: %O', error)
      );
    }, TEMPERATURE_SAVE_DELAY_MS);
  }, []);

  const handleModelChange = useCallback(
    (label: string) => {
      // The current pick is already applied; only the managed default changes
      // anything from the segmented row.
      if (label === MANAGED_LABEL && label !== current) onModelChange?.(null);
    },
    [current, onModelChange]
  );

  const openPicker = useCallback(() => {
    setOpen(false);
    setPickerOpen(true);
  }, []);

  return (
    <>
      <Popover open={open} onOpenChange={handleOpenChange}>
        <PopoverTrigger
          data-testid="composer-chat-settings"
          data-analytics-id="chat-settings-panel"
          aria-label={t('composer.modelSelector')}
          title={t('composer.modelSelector')}
          className="flex h-7 min-w-0 items-center rounded-md px-2 text-xs text-content-muted transition-colors hover:bg-surface-hover hover:text-content">
          <span className="min-w-0 truncate font-medium">
            {loading ? t('composer.settings.loadingModels') : current}
          </span>
          <ChevronDownIcon className="ml-1 size-3.5 shrink-0 opacity-50" aria-hidden />
        </PopoverTrigger>
        <PopoverContent
          side="top"
          align="start"
          className="w-auto bg-transparent p-0 shadow-none ring-0">
          <SettingsPanel
            className="w-80"
            model={current}
            models={models}
            onModelChange={onModelChange ? handleModelChange : undefined}
            temperature={temperature}
            onTemperatureChange={handleTemperatureChange}
            modelLabel={t('composer.settings.model')}
            temperatureLabel={t('composer.settings.temperature')}
            temperatureAriaLabel={t('composer.settings.temperature')}
          />
          {onModelChange && (
            <Button
              type="button"
              variant="tertiary"
              size="xs"
              analyticsId="chat-settings-choose-model"
              className="self-start"
              onClick={openPicker}>
              {t('composer.settings.chooseModel')}
            </Button>
          )}
        </PopoverContent>
      </Popover>
      {pickerOpen && !loading && (
        <ProviderModelPickerDialog
          cloudProviders={providers}
          localModels={NO_LOCAL_MODELS}
          ollamaRunning={false}
          claudeCodeEnabled={false}
          initial={initialSelection}
          onClose={() => setPickerOpen(false)}
          onSelect={selection => {
            onModelChange?.(selectionValue(selection), selection.contextWindow);
            setPickerOpen(false);
          }}
        />
      )}
    </>
  );
}

export default ChatSettingsPanel;
