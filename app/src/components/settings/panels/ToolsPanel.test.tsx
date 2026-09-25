import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import ToolsPanel from './ToolsPanel';

const mocks = vi.hoisted(() => ({ setOnboardingTasks: vi.fn(), useCoreStateMock: vi.fn() }));

vi.mock('../../../providers/CoreStateProvider', () => ({
  useCoreState: () => mocks.useCoreStateMock(),
}));

vi.mock('../../../lib/i18n/I18nContext', () => ({
  useT: () => ({
    t: (key: string) =>
      ({
        'settings.features.tools': 'Tools',
        'pages.settings.features.toolsDesc': 'Tools desc',
        'settings.tools.chooseCapabilities': 'Choose capabilities',
        'settings.tools.saveChanges': 'Save Changes',
        'settings.tools.preferencesSaved': 'Preferences saved',
        'settings.tools.saveFailed': 'Unable to save preferences',
      })[key] ?? key,
  }),
}));

vi.mock('../hooks/useSettingsNavigation', () => ({
  useSettingsNavigation: () => ({ breadcrumbs: [], navigateBack: vi.fn() }),
}));

function coreState(enabledTools: string[]) {
  return {
    snapshot: {
      localState: {
        onboardingTasks: {
          accessibilityPermissionGranted: false,
          localModelConsentGiven: false,
          localModelDownloadStarted: false,
          enabledTools,
          connectedSources: [],
        },
      },
    },
    setOnboardingTasks: mocks.setOnboardingTasks,
  };
}

describe('<ToolsPanel />', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.useCoreStateMock.mockReturnValue(coreState(['shell']));
    mocks.setOnboardingTasks.mockResolvedValue(undefined);
  });

  it('exposes tool toggle state and saves the updated enabled tools list', async () => {
    render(<ToolsPanel />);

    const shellToggle = screen.getByRole('switch', { name: /Shell Commands/ });
    await waitFor(() => expect(shellToggle).toHaveAttribute('aria-checked', 'true'));

    fireEvent.click(shellToggle);

    expect(shellToggle).toHaveAttribute('aria-checked', 'false');
    fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }));

    await waitFor(() =>
      expect(mocks.setOnboardingTasks).toHaveBeenCalledWith(
        expect.objectContaining({ enabledTools: [] })
      )
    );
  });

  it('renders the panel header description when embedded=false (line 110)', () => {
    // Default embedded=false shows the header description
    render(<ToolsPanel embedded={false} />);
    expect(screen.getByText('Tools desc')).toBeInTheDocument();
  });

  it('does not render the panel header when embedded=true (line 101-108 skipped)', () => {
    // When embedded, the header description is not rendered
    render(<ToolsPanel embedded={true} />);
    expect(screen.queryByText('Tools desc')).not.toBeInTheDocument();
  });

  it('shows Save Changes button after toggling a tool (dirty state, line 145-155)', async () => {
    render(<ToolsPanel />);

    const shellToggle = screen.getByRole('switch', { name: /Shell Commands/ });
    await waitFor(() => expect(shellToggle).toHaveAttribute('aria-checked', 'true'));

    // Before toggle — no Save button
    expect(screen.queryByRole('button', { name: 'Save Changes' })).not.toBeInTheDocument();

    // After toggle — dirty=true → Save Changes appears (line 145-155)
    fireEvent.click(shellToggle);
    expect(screen.getByRole('button', { name: 'Save Changes' })).toBeInTheDocument();
  });
});

/**
 * `setOnboardingTasks` takes the WHOLE `StoredOnboardingTasks` record, so this
 * panel has to re-send every flag it does not own. It does that with
 * `onboardingTasks?.<flag> ?? false` (ToolsPanel.tsx, `handleSave`). Every
 * fixture in the suite above sets all of those flags to `false`/`[]`, so a
 * regression that replaced the read-through with a literal `false` would be
 * invisible there. (matrix 2.2.3)
 *
 * Context worth knowing before reading these as "permissions are covered":
 * `accessibilityPermissionGranted` is, at this commit, never written `true` by
 * anything in `app/src` — both writers (this panel and
 * `pages/onboarding/OnboardingLayout.tsx`) read it and write it straight back.
 * Nothing re-derives it from the core's `detect_permissions()`. These tests do
 * not fix that; they make sure that when it is fixed, saving an unrelated
 * settings panel does not silently wipe it again.
 */
describe('<ToolsPanel /> — saving tools preserves the onboarding flags it does not own', () => {
  const populatedCoreState = {
    snapshot: {
      localState: {
        onboardingTasks: {
          accessibilityPermissionGranted: true,
          localModelConsentGiven: true,
          localModelDownloadStarted: true,
          enabledTools: ['shell'],
          connectedSources: ['gmail'],
        },
      },
    },
    setOnboardingTasks: mocks.setOnboardingTasks,
  };

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.useCoreStateMock.mockReturnValue(populatedCoreState);
    mocks.setOnboardingTasks.mockResolvedValue(undefined);
  });

  it('round-trips accessibilityPermissionGranted, model-consent and sources unchanged', async () => {
    render(<ToolsPanel />);

    const shellToggle = screen.getByRole('switch', { name: /Shell Commands/ });
    await waitFor(() => expect(shellToggle).toHaveAttribute('aria-checked', 'true'));

    fireEvent.click(shellToggle);
    fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }));

    await waitFor(() => expect(mocks.setOnboardingTasks).toHaveBeenCalledTimes(1));
    const saved = mocks.setOnboardingTasks.mock.calls[0][0];

    // The flags this panel does not own must survive its save untouched. A
    // hardcoded `false` here silently revokes a recorded macOS permission and
    // a recorded local-model consent every time someone edits tool settings.
    expect(saved.accessibilityPermissionGranted).toBe(true);
    expect(saved.localModelConsentGiven).toBe(true);
    expect(saved.localModelDownloadStarted).toBe(true);
    expect(saved.connectedSources).toEqual(['gmail']);

    // And the thing it does own still changed, so the assertions above are not
    // passing because the save never happened.
    expect(saved.enabledTools).toEqual([]);
  });
});
