import { Navigate, Route, Routes } from 'react-router-dom';

import OnboardingLayout from './OnboardingLayout';
import CustomInferencePage from './pages/CustomInferencePage';
import CustomSearchPage from './pages/CustomSearchPage';
import RuntimeChoicePage from './pages/RuntimeChoicePage';
import VaultSetupStep from './pages/VaultSetupStep';
import WelcomePage from './pages/WelcomePage';

/**
 * Routed onboarding flow.
 *
 *   welcome → runtime-choice
 *     ├── cloud  → /chat
 *     └── custom → /custom/inference → search → vault → /chat
 *
 * Each custom step asks Default (let OpenHuman manage it) vs Configure
 * (let me pick). Default is a one-click pick; Configure renders inline
 * controls (or a deep-link callout to Settings, for domains not yet
 * embedded). A local session skips the choice entirely and always configures.
 *
 * Voice, OAuth and embeddings are retired: six steps before a first message
 * was a forced tour of Settings, and all three stay reachable from Settings.
 * Their page files are kept unrouted so reviving a step is a route entry
 * rather than a rewrite. (An older note here claimed the same for the
 * Gmail/Composio and context steps, but those files are gone — the claim was
 * stale, so treat keeping these as a choice to confirm, not a convention.)
 */
const Onboarding = () => {
  return (
    <Routes>
      <Route element={<OnboardingLayout />}>
        <Route index element={<Navigate to="welcome" replace />} />
        <Route path="welcome" element={<WelcomePage />} />
        <Route path="runtime-choice" element={<RuntimeChoicePage />} />
        <Route path="custom/inference" element={<CustomInferencePage />} />
        <Route path="custom/search" element={<CustomSearchPage />} />
        <Route path="custom/vault" element={<VaultSetupStep />} />
        <Route path="*" element={<Navigate to="welcome" replace />} />
      </Route>
    </Routes>
  );
};

export default Onboarding;
