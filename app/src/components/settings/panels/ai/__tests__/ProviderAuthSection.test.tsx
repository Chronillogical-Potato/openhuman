import { configureStore } from '@reduxjs/toolkit';
import { render, screen } from '@testing-library/react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';

import { I18nProvider } from '../../../../../lib/i18n/I18nContext';
import type { Locale } from '../../../../../lib/i18n/types';
import { CoreStateContext } from '../../../../../providers/coreStateContext';
import localeReducer from '../../../../../store/localeSlice';
import { createLocalSessionToken } from '../../../../../utils/localSession';
import { EMPTY_SETTINGS } from '../aiPanelTypes';
import { ProviderAuthSection } from '../ProviderAuthSection';

function renderSection(sessionToken: string | null) {
  const store = configureStore({
    reducer: { locale: localeReducer },
    preloadedState: { locale: { current: 'en' as Locale } },
  });
  return render(
    <Provider store={store}>
      <I18nProvider>
        <CoreStateContext.Provider value={{ snapshot: { sessionToken } } as never}>
          <ProviderAuthSection
            draft={EMPTY_SETTINGS}
            persist={vi.fn().mockResolvedValue(undefined)}
            loading={false}
            error=""
            busyAction={null}
            providerAuthErrors={[]}
            providerSaveNotice={null}
            onDismissProviderSaveNotice={vi.fn()}
            onProviderRemoved={vi.fn()}
            codexAuthError={null}
            onConnectCodex={vi.fn()}
            onConnectProvider={vi.fn().mockResolvedValue(undefined)}
            onOpenKeyDialog={vi.fn()}
            onAddCustomProvider={vi.fn()}
            onEditCustomProvider={vi.fn()}
          />
        </CoreStateContext.Provider>
      </I18nProvider>
    </Provider>
  );
}

describe('ProviderAuthSection managed row', () => {
  it('shows the managed OpenHuman row for a normal session', () => {
    renderSection('header.payload.signature');
    expect(screen.getByTestId('provider-row-openhuman')).toBeInTheDocument();
  });

  it('hides the managed row for a local ("Continue Locally") session', () => {
    renderSection(createLocalSessionToken());
    expect(screen.getByTestId('provider-group-connected')).toBeInTheDocument();
    expect(screen.queryByTestId('provider-row-openhuman')).not.toBeInTheDocument();
  });

  it('hides the managed row when signed out', () => {
    renderSection(null);
    expect(screen.queryByTestId('provider-row-openhuman')).not.toBeInTheDocument();
  });
});
