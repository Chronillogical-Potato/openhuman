import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { ChatSettingsPanel } from './ChatSettingsPanel';

vi.mock('../../../lib/i18n/I18nContext', () => ({ useT: () => ({ t: (key: string) => key }) }));

vi.mock('../../../components/chat/ModelQualityPill', async importOriginal => {
  const actual = await importOriginal<typeof import('../../../components/chat/ModelQualityPill')>();
  return { ...actual, useModelPickerProviders: () => ({ providers: [], loading: false }) };
});

vi.mock('../../../components/settings/panels/ai/ProviderModelPickerDialog', () => ({
  ProviderModelPickerDialog: ({
    onSelect,
  }: {
    onSelect: (selection: { source: { kind: 'managed' }; model: string }) => void;
  }) => (
    <div data-testid="provider-model-picker-dialog">
      <button
        type="button"
        onClick={() =>
          onSelect({ source: { kind: 'managed' }, model: 'openrouter/openai/gpt-test' })
        }>
        Pick model
      </button>
    </div>
  ),
}));

describe('ChatSettingsPanel', () => {
  it('opens the custom provider/model modal directly from the composer trigger', () => {
    const onModelChange = vi.fn();
    render(<ChatSettingsPanel model={null} onModelChange={onModelChange} />);

    fireEvent.click(screen.getByTestId('composer-chat-settings'));
    expect(screen.getByTestId('provider-model-picker-dialog')).toBeInTheDocument();
    expect(screen.queryByText('composer.settings.chooseModel')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Pick model' }));
    expect(onModelChange).toHaveBeenCalledWith('openrouter/openai/gpt-test', undefined);
  });
});
