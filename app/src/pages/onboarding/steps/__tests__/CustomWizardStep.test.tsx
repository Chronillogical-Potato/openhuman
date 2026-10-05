import { configureStore } from '@reduxjs/toolkit';
import { fireEvent, render, screen, within } from '@testing-library/react';
import type { ComponentProps } from 'react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';

import { I18nProvider } from '../../../../lib/i18n/I18nContext';
import type { Locale } from '../../../../lib/i18n/types';
import localeReducer from '../../../../store/localeSlice';
import { CUSTOM_WIZARD_STEPS } from '../../customWizardSteps';
import CustomWizardStep from '../CustomWizardStep';

function renderStep(
  stepIndex: number,
  extra: Partial<ComponentProps<typeof CustomWizardStep>> = {}
) {
  const store = configureStore({
    reducer: { locale: localeReducer },
    preloadedState: { locale: { current: 'en' as Locale } },
  });
  return render(
    <Provider store={store}>
      <I18nProvider>
        <CustomWizardStep
          stepIndex={stepIndex}
          stepCount={CUSTOM_WIZARD_STEPS.length}
          title="Title"
          subtitle="Subtitle"
          onBack={vi.fn()}
          onContinue={vi.fn()}
          {...extra}
        />
      </I18nProvider>
    </Provider>
  );
}

describe('CustomWizardStep stepper', () => {
  it('labels the three steps Inference, Search and Vault in order', () => {
    renderStep(0);

    const items = within(screen.getByTestId('onboarding-wizard-stepper')).getAllByRole('listitem');
    // Regression guard: a hand-ordered label array sliced to the step count
    // rendered "Inference / Voice / OAuth" here.
    expect(items.map(li => li.textContent?.replace(/^\d+/, '').trim())).toEqual([
      'Inference',
      'Search',
      'Vault',
    ]);
    expect(screen.queryByText('Voice')).not.toBeInTheDocument();
    expect(screen.queryByText('OAuth')).not.toBeInTheDocument();
  });

  it('marks the active step and shows the n-of-total counter', () => {
    renderStep(1);

    const items = within(screen.getByTestId('onboarding-wizard-stepper')).getAllByRole('listitem');
    expect(items[1]).toHaveAttribute('aria-current', 'step');
    expect(items[0]).not.toHaveAttribute('aria-current');
    expect(screen.getByTestId('onboarding-step-counter')).toHaveTextContent('2');
    expect(screen.getByTestId('onboarding-step-counter')).toHaveTextContent('3');
  });
});

describe('CustomWizardStep content', () => {
  // The Default/Configure fork was removed: it re-asked what the runtime choice
  // already answered, and nothing ever read the recorded choice except an
  // analytics tag. The step now always renders its configuration surface.
  it('renders configureContent without any choice being made', () => {
    renderStep(0, { configureContent: <div data-testid="panel">Panel</div> });

    expect(screen.getByTestId('panel')).toBeInTheDocument();
    expect(screen.queryByTestId('onboarding-custom-wizard-step-default')).not.toBeInTheDocument();
    expect(screen.queryByTestId('onboarding-custom-wizard-step-configure')).not.toBeInTheDocument();
  });

  it('enables Continue without a choice and gates it only on continueDisabled', () => {
    const onContinue = vi.fn();
    const { rerender } = renderStep(0, { onContinue });
    const next = screen.getByTestId('onboarding-next-button');
    expect(next).toBeEnabled();
    fireEvent.click(next);
    expect(onContinue).toHaveBeenCalledTimes(1);

    rerender(
      <Provider
        store={configureStore({
          reducer: { locale: localeReducer },
          preloadedState: { locale: { current: 'en' as Locale } },
        })}>
        <I18nProvider>
          <CustomWizardStep
            stepIndex={0}
            stepCount={CUSTOM_WIZARD_STEPS.length}
            title="Title"
            subtitle="Subtitle"
            onBack={vi.fn()}
            onContinue={onContinue}
            continueDisabled
          />
        </I18nProvider>
      </Provider>
    );
    expect(screen.getByTestId('onboarding-next-button')).toBeDisabled();
  });
});
