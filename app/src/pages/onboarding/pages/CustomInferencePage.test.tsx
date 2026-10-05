import { configureStore } from '@reduxjs/toolkit';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { I18nProvider } from '../../../lib/i18n/I18nContext';
import type { Locale } from '../../../lib/i18n/types';
import { CoreStateContext } from '../../../providers/coreStateContext';
import localeReducer from '../../../store/localeSlice';
import CustomInferencePage from './CustomInferencePage';

const navigateMock = vi.fn();
const setDraftMock = vi.fn();
const clearSessionMock = vi.fn().mockResolvedValue(undefined);

vi.mock('react-router-dom', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useNavigate: () => navigateMock };
});

vi.mock('../../../components/settings/panels/AIPanel', () => ({
  default: ({ onDirtyChange }: { onDirtyChange?: (dirty: boolean) => void }) => (
    <div data-testid="ai-panel">
      <button onClick={() => onDirtyChange?.(true)}>make-dirty</button>
      <button onClick={() => onDirtyChange?.(false)}>make-clean</button>
    </div>
  ),
}));

vi.mock('../../../providers/CoreStateProvider', () => ({
  useCoreState: () => ({
    snapshot: { sessionToken: 'header.payload.local' },
    clearSession: clearSessionMock,
  }),
}));

vi.mock('../OnboardingContext', () => ({
  useOnboardingContext: () => ({
    draft: { connectedSources: [] },
    setDraft: setDraftMock,
    completeAndExit: vi.fn(),
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
        <CoreStateContext.Provider
          value={{ snapshot: { sessionToken: 'header.payload.local' } } as never}>
          <I18nProvider>
            <CustomInferencePage />
          </I18nProvider>
        </CoreStateContext.Provider>
      </MemoryRouter>
    </Provider>
  );
}

describe('CustomInferencePage', () => {
  beforeEach(() => {
    navigateMock.mockReset();
    setDraftMock.mockReset();
    clearSessionMock.mockClear();
  });

  it('renders the configuration panel directly with no default/configure chooser', () => {
    renderPage();

    expect(screen.getByTestId('ai-panel')).toBeInTheDocument();
    expect(
      screen.queryByTestId('onboarding-custom-inference-step-default')
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId('onboarding-custom-inference-step-configure')
    ).not.toBeInTheDocument();
  });

  it('waits for clearSession to finish before navigating back to the welcome page', async () => {
    let resolveClearSession!: () => void;
    clearSessionMock.mockReturnValueOnce(
      new Promise<void>(resolve => {
        resolveClearSession = resolve;
      })
    );

    renderPage();

    fireEvent.click(screen.getByRole('button', { name: 'Back' }));

    // Navigation must not race ahead of the session being cleared — otherwise
    // PublicRoute bounces "/" back to /home with the session still live.
    expect(clearSessionMock).toHaveBeenCalledTimes(1);
    expect(navigateMock).not.toHaveBeenCalled();

    resolveClearSession();
    await waitFor(() => expect(navigateMock).toHaveBeenCalledWith('/'));
  });

  it('stays on the step and does not navigate when clearSession fails', async () => {
    clearSessionMock.mockRejectedValueOnce(new Error('clear failed'));

    renderPage();

    fireEvent.click(screen.getByRole('button', { name: 'Back' }));

    await waitFor(() => expect(clearSessionMock).toHaveBeenCalledTimes(1));
    // Give the rejected promise a chance to settle, then confirm we did not
    // navigate to "/" with a still-active session.
    await Promise.resolve();
    expect(navigateMock).not.toHaveBeenCalled();
  });

  it('blocks Continue and shows the hint while the AI panel reports unsaved edits', () => {
    renderPage();

    const next = screen.getByTestId('onboarding-next-button');
    expect(next).toBeEnabled();
    expect(screen.queryByTestId('onboarding-continue-hint')).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('make-dirty'));
    expect(next).toBeDisabled();
    expect(screen.getByTestId('onboarding-continue-hint')).toHaveTextContent(
      'Save your changes before continuing.'
    );

    fireEvent.click(next);
    expect(navigateMock).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText('make-clean'));
    expect(next).toBeEnabled();
    expect(screen.queryByTestId('onboarding-continue-hint')).not.toBeInTheDocument();
  });
});
