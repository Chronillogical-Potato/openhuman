import { configureStore } from '@reduxjs/toolkit';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { I18nProvider } from '../../../lib/i18n/I18nContext';
import type { Locale } from '../../../lib/i18n/types';
import { CoreStateContext } from '../../../providers/coreStateContext';
import localeReducer from '../../../store/localeSlice';
import VaultSetupStep from './VaultSetupStep';

const navigateMock = vi.fn();
const setDraftMock = vi.fn();
const completeAndExitMock = vi.fn();

let sessionToken = 'header.payload.local';

vi.mock('react-router-dom', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useNavigate: () => navigateMock };
});

vi.mock('../../../components/memory/MemoryEngineSetup', () => ({
  default: () => <div data-testid="memory-engine-setup">Memory Engine Setup</div>,
}));

vi.mock('../../../providers/CoreStateProvider', () => ({
  useCoreState: () => ({ snapshot: { sessionToken } }),
}));

vi.mock('../OnboardingContext', () => ({
  useOnboardingContext: () => ({
    draft: { connectedSources: [], customChoices: {} },
    setDraft: setDraftMock,
    completeAndExit: completeAndExitMock,
  }),
}));

function renderPage() {
  const store = configureStore({
    reducer: { locale: localeReducer },
    preloadedState: { locale: { current: 'en' as Locale } },
  });

  return render(
    <Provider store={store}>
      <MemoryRouter>
        <CoreStateContext.Provider value={{ snapshot: { sessionToken } } as never}>
          <I18nProvider>
            <VaultSetupStep />
          </I18nProvider>
        </CoreStateContext.Provider>
      </MemoryRouter>
    </Provider>
  );
}

describe('VaultSetupStep', () => {
  beforeEach(() => {
    navigateMock.mockReset();
    setDraftMock.mockReset();
    completeAndExitMock.mockReset();
    sessionToken = 'header.payload.local';
  });

  it('renders the memory engine setup directly with no default/configure chooser', () => {
    renderPage();

    expect(screen.getByTestId('memory-engine-setup')).toBeInTheDocument();
    expect(screen.queryByTestId('onboarding-custom-vault-step-default')).not.toBeInTheDocument();
    expect(screen.queryByTestId('onboarding-custom-vault-step-configure')).not.toBeInTheDocument();
  });

  it('does the same for managed sessions (the choice fork no longer depends on session)', () => {
    sessionToken = 'header.payload.remote';
    renderPage();

    expect(screen.getByTestId('memory-engine-setup')).toBeInTheDocument();
    expect(screen.queryByTestId('onboarding-custom-vault-step-default')).not.toBeInTheDocument();
  });

  it('finishes setup via completeAndExit on Continue', async () => {
    completeAndExitMock.mockResolvedValue(undefined);
    renderPage();

    fireEvent.click(screen.getByTestId('onboarding-next-button'));

    await waitFor(() => expect(completeAndExitMock).toHaveBeenCalledTimes(1));
  });
});
